//! Ornaments: the part of music21's `expressions` module that changes what
//! is played.
//!
//! A mordent, a trill, a turn, an appoggiatura or a tremolo is written as one
//! note and played as several. Each ornament here knows the note it
//! decorates only through the pitch and the key signature it is given: from
//! those it works out its ornamental pitches -- the neighbour a step above or
//! below, spelled by the key unless the ornament carries an accidental of
//! its own -- and realizes the note as the notes actually played.

use crate::defaults::{FloatType, IntegerType};
use crate::duration::Duration;
use crate::error::{Error, Result};
use crate::interval::{GenericInterval, Interval, IntervalDirection};
use crate::key::keysignature::KeySignature;
use crate::notation::{Tie, TieType};
use crate::note::Note;
use crate::pitch::{Accidental, Pitch};

/// Which ornament a class is, as far as how it is played goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Family {
    /// Nothing to play: the base class and the schleifer.
    Plain,
    Mordent,
    Trill,
    Turn,
    Appoggiatura,
    Tremolo,
}

/// One of music21's ornament classes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum OrnamentKind {
    /// The base class, which plays the note as written.
    Ornament,
    /// An appoggiatura that has not said which way it resolves.
    GeneralAppoggiatura,
    /// A mordent that has not said which way it goes.
    GeneralMordent,
    /// A slide of two notes into the main one.
    Schleifer,
    /// Rapid repetitions of the note.
    Tremolo,
    /// A trill to the note above.
    Trill,
    /// The note above, the note, the note below, the note.
    Turn,
    /// A note a step above, resolving down.
    Appoggiatura,
    /// A trill a half step up.
    HalfStepTrill,
    /// A note a step below, resolving up.
    InvertedAppoggiatura,
    /// The note and the note above.
    InvertedMordent,
    /// A trill to the note below.
    InvertedTrill,
    /// The note below, the note, the note above, the note.
    InvertedTurn,
    /// The note and the note below.
    Mordent,
    /// A trill in shorter notes.
    Shake,
    /// A trill a whole step up.
    WholeStepTrill,
    /// An appoggiatura a half step above.
    HalfStepAppoggiatura,
    /// An appoggiatura a half step below.
    HalfStepInvertedAppoggiatura,
    /// An inverted mordent a half step up.
    HalfStepInvertedMordent,
    /// A mordent a half step down.
    HalfStepMordent,
    /// An appoggiatura a whole step above.
    WholeStepAppoggiatura,
    /// An appoggiatura a whole step below.
    WholeStepInvertedAppoggiatura,
    /// An inverted mordent a whole step up.
    WholeStepInvertedMordent,
    /// A mordent a whole step down.
    WholeStepMordent,
}

/// What an ornament of one kind starts out as.
struct KindRow {
    kind: OrnamentKind,
    class: &'static str,
    /// Its ancestors up to `Ornament`, nearest first.
    parents: &'static [&'static str],
    family: Family,
    /// Whether the ornamental note is above: `Some(true)` up, `Some(false)`
    /// down, `None` for a general class that has not said.
    up: Option<bool>,
    /// The interval a half-step or whole-step ornament always uses, or an
    /// appoggiatura's starting size.
    fixed_size: Option<&'static str>,
    quarter_length: FloatType,
    placement: Option<&'static str>,
    tie_attach: &'static str,
}

const KINDS: [KindRow; 24] = [
    row(
        OrnamentKind::Ornament,
        "Ornament",
        &[],
        Family::Plain,
        None,
        None,
        0.0,
        None,
        "first",
    ),
    row(
        OrnamentKind::GeneralAppoggiatura,
        "GeneralAppoggiatura",
        &["Ornament"],
        Family::Appoggiatura,
        None,
        Some("M2"),
        0.0,
        None,
        "first",
    ),
    row(
        OrnamentKind::GeneralMordent,
        "GeneralMordent",
        &["Ornament"],
        Family::Mordent,
        None,
        None,
        0.125,
        Some("above"),
        "first",
    ),
    row(
        OrnamentKind::Schleifer,
        "Schleifer",
        &["Ornament"],
        Family::Plain,
        None,
        None,
        0.25,
        None,
        "first",
    ),
    row(
        OrnamentKind::Tremolo,
        "Tremolo",
        &["Ornament"],
        Family::Tremolo,
        None,
        None,
        0.0,
        None,
        "first",
    ),
    row(
        OrnamentKind::Trill,
        "Trill",
        &["Ornament"],
        Family::Trill,
        Some(true),
        None,
        0.125,
        Some("above"),
        "all",
    ),
    row(
        OrnamentKind::Turn,
        "Turn",
        &["Ornament"],
        Family::Turn,
        None,
        None,
        0.25,
        Some("above"),
        "all",
    ),
    row(
        OrnamentKind::Appoggiatura,
        "Appoggiatura",
        &["GeneralAppoggiatura", "Ornament"],
        Family::Appoggiatura,
        Some(false),
        Some("M2"),
        0.0,
        None,
        "first",
    ),
    row(
        OrnamentKind::HalfStepTrill,
        "HalfStepTrill",
        &["Trill", "Ornament"],
        Family::Trill,
        Some(true),
        Some("m2"),
        0.125,
        Some("above"),
        "all",
    ),
    row(
        OrnamentKind::InvertedAppoggiatura,
        "InvertedAppoggiatura",
        &["GeneralAppoggiatura", "Ornament"],
        Family::Appoggiatura,
        Some(true),
        Some("M2"),
        0.0,
        None,
        "first",
    ),
    row(
        OrnamentKind::InvertedMordent,
        "InvertedMordent",
        &["GeneralMordent", "Ornament"],
        Family::Mordent,
        Some(true),
        None,
        0.125,
        Some("above"),
        "first",
    ),
    row(
        OrnamentKind::InvertedTrill,
        "InvertedTrill",
        &["Trill", "Ornament"],
        Family::Trill,
        Some(false),
        None,
        0.125,
        Some("above"),
        "all",
    ),
    row(
        OrnamentKind::InvertedTurn,
        "InvertedTurn",
        &["Turn", "Ornament"],
        Family::Turn,
        None,
        None,
        0.25,
        Some("above"),
        "all",
    ),
    row(
        OrnamentKind::Mordent,
        "Mordent",
        &["GeneralMordent", "Ornament"],
        Family::Mordent,
        Some(false),
        None,
        0.125,
        Some("above"),
        "first",
    ),
    row(
        OrnamentKind::Shake,
        "Shake",
        &["Trill", "Ornament"],
        Family::Trill,
        Some(true),
        None,
        0.25,
        Some("above"),
        "all",
    ),
    row(
        OrnamentKind::WholeStepTrill,
        "WholeStepTrill",
        &["Trill", "Ornament"],
        Family::Trill,
        Some(true),
        Some("M2"),
        0.125,
        Some("above"),
        "all",
    ),
    row(
        OrnamentKind::HalfStepAppoggiatura,
        "HalfStepAppoggiatura",
        &["Appoggiatura", "GeneralAppoggiatura", "Ornament"],
        Family::Appoggiatura,
        Some(false),
        Some("m2"),
        0.0,
        None,
        "first",
    ),
    row(
        OrnamentKind::HalfStepInvertedAppoggiatura,
        "HalfStepInvertedAppoggiatura",
        &["InvertedAppoggiatura", "GeneralAppoggiatura", "Ornament"],
        Family::Appoggiatura,
        Some(true),
        Some("m2"),
        0.0,
        None,
        "first",
    ),
    row(
        OrnamentKind::HalfStepInvertedMordent,
        "HalfStepInvertedMordent",
        &["InvertedMordent", "GeneralMordent", "Ornament"],
        Family::Mordent,
        Some(true),
        Some("m2"),
        0.125,
        Some("above"),
        "first",
    ),
    row(
        OrnamentKind::HalfStepMordent,
        "HalfStepMordent",
        &["Mordent", "GeneralMordent", "Ornament"],
        Family::Mordent,
        Some(false),
        Some("m-2"),
        0.125,
        Some("above"),
        "first",
    ),
    row(
        OrnamentKind::WholeStepAppoggiatura,
        "WholeStepAppoggiatura",
        &["Appoggiatura", "GeneralAppoggiatura", "Ornament"],
        Family::Appoggiatura,
        Some(false),
        Some("M2"),
        0.0,
        None,
        "first",
    ),
    row(
        OrnamentKind::WholeStepInvertedAppoggiatura,
        "WholeStepInvertedAppoggiatura",
        &["InvertedAppoggiatura", "GeneralAppoggiatura", "Ornament"],
        Family::Appoggiatura,
        Some(true),
        Some("M2"),
        0.0,
        None,
        "first",
    ),
    row(
        OrnamentKind::WholeStepInvertedMordent,
        "WholeStepInvertedMordent",
        &["InvertedMordent", "GeneralMordent", "Ornament"],
        Family::Mordent,
        Some(true),
        Some("M2"),
        0.125,
        Some("above"),
        "first",
    ),
    row(
        OrnamentKind::WholeStepMordent,
        "WholeStepMordent",
        &["Mordent", "GeneralMordent", "Ornament"],
        Family::Mordent,
        Some(false),
        Some("M-2"),
        0.125,
        Some("above"),
        "first",
    ),
];

#[allow(clippy::too_many_arguments)]
const fn row(
    kind: OrnamentKind,
    class: &'static str,
    parents: &'static [&'static str],
    family: Family,
    up: Option<bool>,
    fixed_size: Option<&'static str>,
    quarter_length: FloatType,
    placement: Option<&'static str>,
    tie_attach: &'static str,
) -> KindRow {
    KindRow {
        kind,
        class,
        parents,
        family,
        up,
        fixed_size,
        quarter_length,
        placement,
        tie_attach,
    }
}

impl OrnamentKind {
    /// Every kind, parents before children.
    pub const ALL: [OrnamentKind; 24] = [
        OrnamentKind::Ornament,
        OrnamentKind::GeneralAppoggiatura,
        OrnamentKind::GeneralMordent,
        OrnamentKind::Schleifer,
        OrnamentKind::Tremolo,
        OrnamentKind::Trill,
        OrnamentKind::Turn,
        OrnamentKind::Appoggiatura,
        OrnamentKind::HalfStepTrill,
        OrnamentKind::InvertedAppoggiatura,
        OrnamentKind::InvertedMordent,
        OrnamentKind::InvertedTrill,
        OrnamentKind::InvertedTurn,
        OrnamentKind::Mordent,
        OrnamentKind::Shake,
        OrnamentKind::WholeStepTrill,
        OrnamentKind::HalfStepAppoggiatura,
        OrnamentKind::HalfStepInvertedAppoggiatura,
        OrnamentKind::HalfStepInvertedMordent,
        OrnamentKind::HalfStepMordent,
        OrnamentKind::WholeStepAppoggiatura,
        OrnamentKind::WholeStepInvertedAppoggiatura,
        OrnamentKind::WholeStepInvertedMordent,
        OrnamentKind::WholeStepMordent,
    ];

    fn row(self) -> &'static KindRow {
        KINDS
            .iter()
            .find(|row| row.kind == self)
            .expect("every kind has a row")
    }

    /// music21's class name for the kind: `"InvertedMordent"`.
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

    /// The classes above this one, nearest first, up to `Ornament`.
    pub fn parents(self) -> &'static [&'static str] {
        self.row().parents
    }

    /// Whether the kind always uses one interval, as the half-step and
    /// whole-step mordents and trills do, and so takes no accidental.
    pub fn has_fixed_size(self) -> bool {
        self.row().fixed_size.is_some()
            && matches!(self.row().family, Family::Mordent | Family::Trill)
    }
}

/// When a turn starts: music21's `OrnamentDelay`.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum OrnamentDelay {
    /// At once, on the beat.
    NoDelay,
    /// Halfway through the note.
    Default,
    /// After so many quarter lengths.
    Timed(FloatType),
}

/// Which neighbour of a turn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TurnNote {
    /// The note above.
    Upper,
    /// The note below.
    Lower,
}

/// A note realized as an ornament plays it: the notes before what is left of
/// it, what is left (nothing where the ornament takes the whole note, as a
/// trill does), and the notes after.
#[derive(Clone, Debug)]
pub struct Realization {
    /// The notes played before the main note.
    pub before: Vec<Note>,
    /// What is left of the main note.
    pub main: Option<Note>,
    /// The notes played after it.
    pub after: Vec<Note>,
}

impl Realization {
    /// Every note in the order played.
    pub fn into_notes(self) -> Vec<Note> {
        let mut notes = self.before;
        notes.extend(self.main);
        notes.extend(self.after);
        notes
    }
}

/// An ornament.
///
/// ```
/// use music21_rs::expressions::{Ornament, OrnamentKind};
/// use music21_rs::{KeySignature, Note};
///
/// let mordent = Ornament::of_kind(OrnamentKind::Mordent);
/// let played = mordent.realize(&Note::from_name("C4")?, &KeySignature::new(0))?;
/// let names: Vec<String> = played
///     .into_notes()
///     .iter()
///     .map(|note| {
///         let length = note.duration().map_or(1.0, |duration| duration.quarter_length());
///         format!("{} {length}", note.pitch().name_with_octave())
///     })
///     .collect();
/// assert_eq!(names, ["C4 0.125", "B3 0.125", "C4 0.75"]);
/// # Ok::<(), music21_rs::Error>(())
/// ```
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[must_use]
pub struct Ornament {
    kind: OrnamentKind,
    quarter_length: FloatType,
    auto_scale: bool,
    placement: Option<String>,
    tie_attach: String,
    accidental: Option<Accidental>,
    upper_accidental: Option<Accidental>,
    lower_accidental: Option<Accidental>,
    delay: OrnamentDelay,
    nachschlag: bool,
    number_of_marks: u8,
    measured: bool,
    size: Option<Interval>,
}

impl Default for Ornament {
    /// music21's bare `Ornament`.
    fn default() -> Self {
        Self::of_kind(OrnamentKind::Ornament)
    }
}

impl Ornament {
    /// An ornament as music21's class for `kind` starts out.
    pub fn of_kind(kind: OrnamentKind) -> Self {
        let row = kind.row();
        let size = match row.family {
            Family::Appoggiatura => row
                .fixed_size
                .map(|name| Interval::from_name(name).expect("a fixed interval name")),
            _ => None,
        };
        Self {
            kind,
            quarter_length: row.quarter_length,
            auto_scale: true,
            placement: row.placement.map(str::to_string),
            tie_attach: row.tie_attach.to_string(),
            accidental: None,
            upper_accidental: None,
            lower_accidental: None,
            delay: OrnamentDelay::NoDelay,
            nachschlag: false,
            number_of_marks: 3,
            measured: true,
            size,
        }
    }

    /// The kind of ornament this started as.
    pub fn kind(&self) -> OrnamentKind {
        self.kind
    }

    /// Whether this is an ornament of that music21 class or one below it.
    pub fn is_a(&self, class: &str) -> bool {
        self.kind.class_name() == class || self.kind.parents().contains(&class)
    }

    fn family(&self) -> Family {
        self.kind.row().family
    }

    /// Which way a mordent, trill or appoggiatura goes to its ornamental
    /// note: music21's `direction`. Nothing for a general mordent or
    /// appoggiatura that has not said, and for an ornament of another kind.
    pub fn direction(&self) -> Option<IntervalDirection> {
        match self.family() {
            Family::Mordent | Family::Trill | Family::Appoggiatura => {
                self.kind.row().up.map(|up| {
                    if up {
                        IntervalDirection::Ascending
                    } else {
                        IntervalDirection::Descending
                    }
                })
            }
            _ => None,
        }
    }

    /// music21's `name`: the class as lower-case words, with a mordent's or
    /// trill's accidental, and a turn's delay and accidentals, beside it.
    pub fn name(&self) -> String {
        let mut name =
            crate::common::stringtools::camel_case_to_hyphen(self.kind.class_name(), ' ');
        match self.family() {
            Family::Mordent | Family::Trill => {
                if let Some(accidental) = &self.accidental {
                    name = format!("{name} ({})", accidental.name());
                }
            }
            Family::Turn => {
                match self.delay {
                    OrnamentDelay::Default => name = format!("delayed {name}"),
                    OrnamentDelay::Timed(delay) => {
                        name = format!("delayed(delayQL={}) {name}", python_float(delay));
                    }
                    OrnamentDelay::NoDelay => {}
                }
                let parts: Vec<String> = [
                    self.upper_accidental
                        .as_ref()
                        .map(|accidental| format!("upper={}", accidental.name())),
                    self.lower_accidental
                        .as_ref()
                        .map(|accidental| format!("lower={}", accidental.name())),
                ]
                .into_iter()
                .flatten()
                .collect();
                if !parts.is_empty() {
                    name = format!("{name} ({})", parts.join(", "));
                }
            }
            _ => {}
        }
        name
    }

    /// How long each ornamental note lasts, in quarter lengths: music21's
    /// `quarterLength`.
    pub fn quarter_length(&self) -> FloatType {
        self.quarter_length
    }

    /// Changes how long each ornamental note lasts.
    pub fn set_quarter_length(&mut self, quarter_length: FloatType) {
        self.quarter_length = quarter_length;
    }

    /// Whether the ornament shrinks to fit a note too short for it: music21's
    /// `autoScale`.
    pub fn auto_scale(&self) -> bool {
        self.auto_scale
    }

    /// Says whether the ornament shrinks to fit.
    pub fn set_auto_scale(&mut self, auto_scale: bool) {
        self.auto_scale = auto_scale;
    }

    /// Where the sign sits.
    pub fn placement(&self) -> Option<&str> {
        self.placement.as_deref()
    }

    /// Says where the sign sits.
    pub fn set_placement(&mut self, placement: Option<String>) {
        self.placement = placement;
    }

    /// Which part of a note split across a tie keeps the ornament.
    pub fn tie_attach(&self) -> &str {
        &self.tie_attach
    }

    /// Changes which part keeps it.
    pub fn set_tie_attach(&mut self, attach: impl Into<String>) {
        self.tie_attach = attach.into();
    }

    /// A mordent's or trill's accidental, which spells its ornamental note
    /// instead of the key signature.
    pub fn accidental(&self) -> Option<&Accidental> {
        self.accidental.as_ref()
    }

    /// Changes the accidental.
    ///
    /// # Errors
    ///
    /// A half-step or whole-step ornament, whose interval is fixed.
    pub fn set_accidental(&mut self, accidental: Option<Accidental>) -> Result<()> {
        if self.kind.has_fixed_size() {
            return Err(Error::Expression(format!(
                "Cannot set accidental of {}",
                self.kind.class_name()
            )));
        }
        self.accidental = accidental;
        Ok(())
    }

    /// A turn's accidental for its upper note.
    pub fn upper_accidental(&self) -> Option<&Accidental> {
        self.upper_accidental.as_ref()
    }

    /// Changes a turn's upper accidental.
    pub fn set_upper_accidental(&mut self, accidental: Option<Accidental>) {
        self.upper_accidental = accidental;
    }

    /// A turn's accidental for its lower note.
    pub fn lower_accidental(&self) -> Option<&Accidental> {
        self.lower_accidental.as_ref()
    }

    /// Changes a turn's lower accidental.
    pub fn set_lower_accidental(&mut self, accidental: Option<Accidental>) {
        self.lower_accidental = accidental;
    }

    /// When a turn starts.
    pub fn delay(&self) -> OrnamentDelay {
        self.delay
    }

    /// Changes when a turn starts; a timed delay of nothing is no delay.
    pub fn set_delay(&mut self, delay: OrnamentDelay) {
        self.delay = match delay {
            OrnamentDelay::Timed(length) if length <= 0.0 => OrnamentDelay::NoDelay,
            other => other,
        };
    }

    /// Whether a turn waits before starting.
    pub fn is_delayed(&self) -> bool {
        self.delay != OrnamentDelay::NoDelay
    }

    /// Whether a trill ends on two notes turning back into the next:
    /// music21's `nachschlag`.
    pub fn nachschlag(&self) -> bool {
        self.nachschlag
    }

    /// Says whether a trill ends on a nachschlag.
    pub fn set_nachschlag(&mut self, nachschlag: bool) {
        self.nachschlag = nachschlag;
    }

    /// How many strokes cross a tremolo's stem, nought to eight.
    pub fn number_of_marks(&self) -> u8 {
        self.number_of_marks
    }

    /// Changes how many strokes cross the stem.
    ///
    /// # Errors
    ///
    /// More than eight.
    pub fn set_number_of_marks(&mut self, marks: u8) -> Result<()> {
        if marks > 8 {
            return Err(Error::Expression(
                "Number of marks must be a number from 0 to 8".to_string(),
            ));
        }
        self.number_of_marks = marks;
        Ok(())
    }

    /// Whether a tremolo is measured.
    pub fn measured(&self) -> bool {
        self.measured
    }

    /// Says whether a tremolo is measured.
    pub fn set_measured(&mut self, measured: bool) {
        self.measured = measured;
    }

    /// An appoggiatura's size: how far from the main note its first note is.
    pub fn appoggiatura_size(&self) -> Option<&Interval> {
        self.size.as_ref()
    }

    /// Changes an appoggiatura's size.
    pub fn set_appoggiatura_size(&mut self, size: Option<Interval>) {
        self.size = size;
    }

    /// The interval from `source` to a mordent's or trill's ornamental note,
    /// under `key`: music21's `getSize`.
    ///
    /// ```
    /// use music21_rs::expressions::{Ornament, OrnamentKind};
    /// use music21_rs::{KeySignature, Pitch};
    ///
    /// let mordent = Ornament::of_kind(OrnamentKind::Mordent);
    /// let g = Pitch::from_name("G4")?;
    /// assert_eq!(mordent.size(&g, &KeySignature::new(0))?.directed_name(), "M-2");
    /// assert_eq!(mordent.size(&g, &KeySignature::new(1))?.directed_name(), "m-2");
    /// # Ok::<(), music21_rs::Error>(())
    /// ```
    ///
    /// # Errors
    ///
    /// An ornament that is not a mordent or a trill, or a general mordent
    /// that has not said which way it goes.
    pub fn size(&self, source: &Pitch, key: &KeySignature) -> Result<Interval> {
        let row = self.kind.row();
        let what = match row.family {
            Family::Mordent => "mordent",
            Family::Trill => "trill",
            _ => {
                return Err(Error::Expression(format!(
                    "a {} has no single size",
                    self.name()
                )));
            }
        };
        if let Some(name) = row.fixed_size {
            return Interval::from_name(name);
        }
        let Some(up) = row.up else {
            return Err(Error::Expression(format!(
                "Cannot compute {what} size if I do not know its direction"
            )));
        };
        neighbour(source, up, self.accidental.as_ref(), key)
    }

    /// The interval from `source` to a turn's upper or lower note, under
    /// `key`: music21's `Turn.getSize`.
    ///
    /// # Errors
    ///
    /// An ornament that is not a turn.
    pub fn turn_size(
        &self,
        source: &Pitch,
        which: TurnNote,
        key: &KeySignature,
    ) -> Result<Interval> {
        if self.family() != Family::Turn {
            return Err(Error::Expression(format!(
                "a {} is not a turn",
                self.name()
            )));
        }
        match which {
            TurnNote::Upper => neighbour(source, true, self.upper_accidental.as_ref(), key),
            TurnNote::Lower => neighbour(source, false, self.lower_accidental.as_ref(), key),
        }
    }

    /// The pitches the ornament plays besides `source` itself, under `key`:
    /// music21's `resolveOrnamentalPitches` and `ornamentalPitches`. A
    /// mordent or trill has one, a turn its upper and lower notes in that
    /// order, and any other ornament none. An ornament's own accidental
    /// passes its display status on to the pitch it spells.
    ///
    /// # Errors
    ///
    /// A general mordent that has not said which way it goes.
    pub fn ornamental_pitches(&self, source: &Pitch, key: &KeySignature) -> Result<Vec<Pitch>> {
        let resolved = |interval: &Interval, accidental: Option<&Accidental>| -> Result<Pitch> {
            let mut from = source.clone();
            from.set_octave_is_implicit(false);
            let mut ornamental = from.transpose(interval)?;
            if ornamental
                .microtone()
                .is_some_and(|microtone| microtone.cents() != 0.0)
            {
                ornamental = ornamental.convert_microtones_to_quarter_tones()?;
            }
            if let Some(accidental) = accidental {
                let status = accidental.display_status();
                let written = ornamental
                    .accidental()
                    .cloned()
                    .unwrap_or_else(|| Accidental::new("natural").expect("a natural"));
                let mut written = written;
                written.set_display_status(status);
                ornamental.set_accidental(Some(written));
            }
            Ok(ornamental)
        };
        match self.family() {
            Family::Mordent | Family::Trill => {
                let interval = self.size(source, key)?;
                Ok(vec![resolved(&interval, self.accidental.as_ref())?])
            }
            Family::Turn => {
                let upper = self.turn_size(source, TurnNote::Upper, key)?;
                let lower = self.turn_size(source, TurnNote::Lower, key)?;
                Ok(vec![
                    resolved(&upper, self.upper_accidental.as_ref())?,
                    resolved(&lower, self.lower_accidental.as_ref())?,
                ])
            }
            _ => Ok(Vec::new()),
        }
    }

    /// The notes `note` is played as under this ornament, in `key`:
    /// music21's `realize`.
    ///
    /// # Errors
    ///
    /// A note with no length to steal time from (a tremolo takes one as it
    /// is), a note too short for an ornament told not to shrink, or a general
    /// mordent or appoggiatura that has not said which way it goes.
    pub fn realize(&self, note: &Note, key: &KeySignature) -> Result<Realization> {
        // A note that says no length lasts a quarter, as music21's does.
        let length = note.duration().map_or(1.0, Duration::quarter_length);
        match self.family() {
            Family::Plain => Ok(Realization {
                before: Vec::new(),
                main: Some(note.clone()),
                after: Vec::new(),
            }),
            Family::Mordent => self.realize_mordent(note, key, length),
            Family::Trill => self.realize_trill(note, key, length),
            Family::Turn => self.realize_turn(note, key, length),
            Family::Appoggiatura => self.realize_appoggiatura(note, length),
            Family::Tremolo => self.realize_tremolo(note, length),
        }
    }

    fn realize_mordent(
        &self,
        note: &Note,
        key: &KeySignature,
        length: FloatType,
    ) -> Result<Realization> {
        if self.kind.row().up.is_none() {
            return Err(Error::Expression(
                "Cannot realize a mordent if I do not know its direction".to_string(),
            ));
        }
        no_time_to_steal(length)?;
        let mut each = self.quarter_length;
        if length <= self.quarter_length * 2.0 {
            if !self.auto_scale {
                return Err(Error::Expression(
                    "The note is not long enough to realize a mordent".to_string(),
                ));
            }
            each = length / 4.0;
        }
        let interval = self.size(note.pitch(), key)?;
        let mut second = lasting(note, each)?;
        let mut moved = second.pitch().transpose(&interval)?;
        if moved.accidental().is_none() {
            moved.set_accidental(key.accidental_by_step(moved.step().as_char())?);
        }
        second.set_pitch(moved);
        Ok(Realization {
            before: vec![lasting(note, each)?, second],
            main: Some(lasting(note, length - 2.0 * each)?),
            after: Vec::new(),
        })
    }

    fn realize_trill(
        &self,
        note: &Note,
        key: &KeySignature,
        length: FloatType,
    ) -> Result<Realization> {
        no_time_to_steal(length)?;
        let mut each = self.quarter_length;
        if length < 2.0 * each {
            if !self.auto_scale {
                return Err(Error::Expression(
                    "The note is not long enough to realize a trill".to_string(),
                ));
            }
            each = length / 2.0;
        }
        if length < 4.0 * self.quarter_length && self.nachschlag {
            if !self.auto_scale {
                return Err(Error::Expression(
                    "The note is not long enough for a nachschlag".to_string(),
                ));
            }
            each = length / 4.0;
        }
        let interval = self.size(note.pitch(), key)?;
        let from_key = !self.kind.has_fixed_size();
        let mut count = (length / each) as IntegerType;
        if self.nachschlag {
            count -= 2;
        }
        let written = note.pitch().name_with_octave();
        let mut trilled = Vec::new();
        for _ in 0..(count / 2) {
            trilled.push(lasting(note, each)?);
            let mut upper = lasting(note, each)?;
            upper.set_pitch(upper.pitch().transpose(&interval)?);
            trilled.push(upper);
        }
        if from_key {
            for played in &mut trilled {
                if played.pitch().name_with_octave() != written
                    && played.pitch().accidental().is_none()
                {
                    let mut pitch = played.pitch().clone();
                    pitch.set_accidental(key.accidental_by_step(pitch.step().as_char())?);
                    played.set_pitch(pitch);
                }
            }
        }
        if !self.nachschlag {
            return Ok(Realization {
                before: trilled,
                main: None,
                after: Vec::new(),
            });
        }
        let mut first = lasting(note, each)?;
        let mut second = lasting(note, each)?;
        second.set_pitch(second.pitch().transpose(&interval.reversed()?)?);
        if from_key {
            for played in [&mut first, &mut second] {
                let mut pitch = played.pitch().clone();
                pitch.set_accidental(key.accidental_by_step(pitch.step().as_char())?);
                played.set_pitch(pitch);
            }
        }
        Ok(Realization {
            before: trilled,
            main: None,
            after: vec![first, second],
        })
    }

    fn realize_turn(
        &self,
        note: &Note,
        key: &KeySignature,
        length: FloatType,
    ) -> Result<Realization> {
        no_time_to_steal(length)?;
        let remainder = match self.delay {
            OrnamentDelay::NoDelay => 0.0,
            OrnamentDelay::Default => length / 2.0,
            OrnamentDelay::Timed(delay) => delay,
        };
        let turn_length = length - remainder;
        let mut each = self.quarter_length;
        let mut last = None;
        if turn_length < 4.0 * self.quarter_length {
            if !self.auto_scale {
                return Err(Error::Expression(
                    "The note is not long enough to realize a turn".to_string(),
                ));
            }
            each = turn_length / 4.0;
        } else if turn_length > 4.0 * self.quarter_length {
            last = Some(turn_length - 3.0 * each);
        }
        let inverted = self.kind == OrnamentKind::InvertedTurn;
        let (first_side, third_side) = if inverted {
            (TurnNote::Lower, TurnNote::Upper)
        } else {
            (TurnNote::Upper, TurnNote::Lower)
        };
        let first_interval = self.turn_size(note.pitch(), first_side, key)?;
        let third_interval = self.turn_size(note.pitch(), third_side, key)?;
        let mut first = lasting(note, each)?;
        first.set_pitch(first.pitch().transpose(&first_interval)?);
        let second = lasting(note, each)?;
        let mut third = lasting(note, each)?;
        third.set_pitch(third.pitch().transpose(&third_interval)?);
        let fourth = lasting(note, last.unwrap_or(each))?;
        let mut turned = vec![first, second, third, fourth];
        for index in [0, 2] {
            if turned[index].pitch().accidental().is_none() {
                let mut pitch = turned[index].pitch().clone();
                pitch.set_accidental(key.accidental_by_step(pitch.step().as_char())?);
                turned[index].set_pitch(pitch);
            }
        }
        if remainder == 0.0 {
            return Ok(Realization {
                before: Vec::new(),
                main: None,
                after: turned,
            });
        }
        Ok(Realization {
            before: Vec::new(),
            main: Some(lasting(note, remainder)?),
            after: turned,
        })
    }

    fn realize_appoggiatura(&self, note: &Note, length: FloatType) -> Result<Realization> {
        let Some(up) = self.kind.row().up else {
            return Err(Error::Expression(
                "Cannot realize an Appoggiatura if I do not know its direction".to_string(),
            ));
        };
        let Some(size) = &self.size else {
            return Err(Error::Expression(
                "Cannot realize an Appoggiatura if there is no size given".to_string(),
            ));
        };
        no_time_to_steal(length)?;
        let half = length / 2.0;
        let interval = if up { size.reversed()? } else { size.clone() };
        let mut leaning = lasting(note, half)?;
        leaning.set_pitch(leaning.pitch().transpose(&interval)?);
        Ok(Realization {
            before: vec![leaning],
            main: Some(lasting(note, half)?),
            after: Vec::new(),
        })
    }

    fn realize_tremolo(&self, note: &Note, length: FloatType) -> Result<Realization> {
        let each = (0.5 as FloatType).powi(IntegerType::from(self.number_of_marks));
        let mut repeated = Vec::new();
        let mut remaining = note.clone();
        let mut left = length;
        while left > each {
            let mut played = lasting(&remaining, each)?;
            left -= each;
            remaining = lasting(&remaining, left)?;
            tie_split(&mut played, &mut remaining);
            repeated.push(played);
        }
        repeated.push(remaining);
        Ok(Realization {
            before: repeated,
            main: None,
            after: Vec::new(),
        })
    }
}

/// Ties the two halves of a note split in two, as music21's
/// `splitAtQuarterLength` does: the halves are tied to each other, and a tie
/// the note already had carries on across them.
fn tie_split(first: &mut Note, rest: &mut Note) {
    let rest_type = match first.tie().map(Tie::tie_type) {
        None => {
            first.set_tie(Some(Tie::new(TieType::Start)));
            TieType::Stop
        }
        Some(TieType::Stop) => {
            first.set_tie(Some(Tie::new(TieType::Continue)));
            TieType::Stop
        }
        Some(TieType::Start | TieType::Continue) => TieType::Continue,
        Some(_) => TieType::Stop,
    };
    rest.set_tie(Some(Tie::new(rest_type)));
}

/// Refuses a note with no length for an ornament to take its notes out of.
fn no_time_to_steal(length: FloatType) -> Result<()> {
    if length == 0.0 {
        return Err(Error::Expression(
            "Cannot steal time from an object with no duration".to_string(),
        ));
    }
    Ok(())
}

/// `note` lasting `quarter_length`.
fn lasting(note: &Note, quarter_length: FloatType) -> Result<Note> {
    let mut copy = note.clone();
    copy.set_duration(Duration::new(quarter_length)?);
    Ok(copy)
}

/// The interval from `source` to the step above or below it, spelled by
/// `accidental` where there is one and by `key` where not: how music21's
/// mordents, trills and turns size themselves.
fn neighbour(
    source: &Pitch,
    up: bool,
    accidental: Option<&Accidental>,
    key: &KeySignature,
) -> Result<Interval> {
    let mut ornamental = source.clone();
    ornamental.set_accidental(None);
    ornamental.set_octave_is_implicit(false);
    let mut ornamental =
        GenericInterval::new(if up { 2 } else { -2 })?.transpose_pitch(&ornamental)?;
    let spelled = match accidental {
        Some(accidental) => Some(accidental.clone()),
        None => key.accidental_by_step(ornamental.step().as_char())?,
    };
    ornamental.set_accidental(spelled);
    Interval::between_pitches(source, &ornamental)
}

/// A number written as Python writes a float: `0.5`, `1.0`.
fn python_float(value: FloatType) -> String {
    if value.fract() == 0.0 {
        format!("{value:.1}")
    } else {
        value.to_string()
    }
}

/// Every ornament on `note` played in turn, as music21's
/// `realizeOrnaments` plays a note's expressions: each ornament realizes
/// what the one before left of the note, and an ornament that takes the
/// whole note ends the walk.
///
/// # Errors
///
/// Any ornament that cannot realize what it is given.
pub fn realize_ornaments(
    note: &Note,
    ornaments: &[Ornament],
    key: &KeySignature,
) -> Result<Vec<Note>> {
    let mut before = Vec::new();
    let mut after: Vec<Note> = Vec::new();
    let mut current = Some(note.clone());
    for ornament in ornaments {
        let Some(main) = current.take() else {
            break;
        };
        let played = ornament.realize(&main, key)?;
        before.extend(played.before);
        after.extend(played.after);
        current = played.main;
    }
    before.extend(current);
    before.extend(after);
    Ok(before)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn written(notes: Vec<Note>) -> Vec<String> {
        notes
            .iter()
            .map(|note| {
                let length = note.duration().map_or(1.0, Duration::quarter_length);
                match note.tie() {
                    Some(tie) => format!(
                        "{} {length} {}",
                        note.pitch().name_with_octave(),
                        tie.tie_type().as_str()
                    ),
                    None => format!("{} {length}", note.pitch().name_with_octave()),
                }
            })
            .collect()
    }

    /// Every answer here is music21's own.
    #[test]
    fn an_ornament_answers_what_music21_s_answers() {
        let directions = [
            (OrnamentKind::GeneralMordent, None),
            (OrnamentKind::Mordent, Some(IntervalDirection::Descending)),
            (
                OrnamentKind::InvertedMordent,
                Some(IntervalDirection::Ascending),
            ),
            (OrnamentKind::Trill, Some(IntervalDirection::Ascending)),
            (
                OrnamentKind::InvertedTrill,
                Some(IntervalDirection::Descending),
            ),
            (OrnamentKind::Shake, Some(IntervalDirection::Ascending)),
            (OrnamentKind::GeneralAppoggiatura, None),
            (
                OrnamentKind::Appoggiatura,
                Some(IntervalDirection::Descending),
            ),
            (
                OrnamentKind::InvertedAppoggiatura,
                Some(IntervalDirection::Ascending),
            ),
            (OrnamentKind::Turn, None),
        ];
        for (kind, direction) in directions {
            assert_eq!(Ornament::of_kind(kind).direction(), direction, "{kind:?}");
        }

        let key = KeySignature::new(0);
        let mut quarter = Note::from_name("C4").unwrap();
        quarter.set_duration(Duration::new(1.0).unwrap());
        let tremolo = Ornament::of_kind(OrnamentKind::Tremolo);
        let played = tremolo.realize(&quarter, &key).unwrap().into_notes();
        assert_eq!(played.len(), 8);
        assert_eq!(
            written(played)[..2],
            ["C4 0.125 start", "C4 0.125 continue"]
        );

        let mut half_step = Ornament::of_kind(OrnamentKind::HalfStepMordent);
        assert!(
            half_step
                .set_accidental(Accidental::new("sharp").ok())
                .is_err()
        );

        let mut trill = Ornament::of_kind(OrnamentKind::Trill);
        trill.set_nachschlag(true);
        let played = written(trill.realize(&quarter, &key).unwrap().into_notes());
        assert_eq!(
            played,
            [
                "C4 0.125", "D4 0.125", "C4 0.125", "D4 0.125", "C4 0.125", "D4 0.125", "C4 0.125",
                "B3 0.125"
            ]
        );
    }
}
