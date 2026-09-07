//! Notation attached to a note or a chord: ties, noteheads, stem direction,
//! colour and lyrics.
//!
//! These are the parts of music21's `note.NotRest` and `note.GeneralNote`
//! that carry musical intent rather than layout. A tie says two written notes
//! sound as one; a diamond notehead says a string harmonic; a lyric is the
//! text sung on the note. None of them need a stream to mean something, which
//! is why they live in the data model here alongside pitch and duration.

use std::fmt;
use std::str::FromStr;

use crate::{
    defaults::IntegerType,
    duration::DurationType,
    error::{Error, Result},
};

/// How a tie joins this note to its neighbours: music21's `tie.Tie.type`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum TieType {
    /// The note begins a tie.
    Start,
    /// The note ends a tie.
    Stop,
    /// The note is tied both from the one before and to the one after.
    Continue,
    /// The note rings on with no written end, as a piano pedal note does.
    LetRing,
    /// A `continue` that also lets the sound ring on.
    ContinueLetRing,
}

impl TieType {
    /// Every tie type, in music21's order.
    pub const ALL: [TieType; 5] = [
        TieType::Start,
        TieType::Stop,
        TieType::Continue,
        TieType::LetRing,
        TieType::ContinueLetRing,
    ];

    /// music21's name for the type.
    pub fn as_str(self) -> &'static str {
        match self {
            TieType::Start => "start",
            TieType::Stop => "stop",
            TieType::Continue => "continue",
            TieType::LetRing => "let-ring",
            TieType::ContinueLetRing => "continue-let-ring",
        }
    }

    /// Reads music21's name for the type.
    pub fn from_name(name: &str) -> Result<Self> {
        Self::ALL
            .into_iter()
            .find(|candidate| candidate.as_str() == name)
            .ok_or_else(|| {
                let valid = Self::ALL
                    .iter()
                    .map(|candidate| format!("'{}'", candidate.as_str()))
                    .collect::<Vec<_>>()
                    .join(", ");
                Error::Notation(format!("Type must be one of ({valid}), not {name}"))
            })
    }
}

impl fmt::Display for TieType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// How the tie is drawn: music21's `tie.Tie.style`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum TieStyle {
    /// A solid tie.
    #[default]
    Normal,
    /// A dotted tie.
    Dotted,
    /// A dashed tie.
    Dashed,
    /// A tie that sounds but is not drawn.
    Hidden,
}

impl TieStyle {
    /// Every tie style.
    pub const ALL: [TieStyle; 4] = [
        TieStyle::Normal,
        TieStyle::Dotted,
        TieStyle::Dashed,
        TieStyle::Hidden,
    ];

    /// music21's name for the style.
    pub fn as_str(self) -> &'static str {
        match self {
            TieStyle::Normal => "normal",
            TieStyle::Dotted => "dotted",
            TieStyle::Dashed => "dashed",
            TieStyle::Hidden => "hidden",
        }
    }

    /// Reads music21's name for the style.
    pub fn from_name(name: &str) -> Result<Self> {
        Self::ALL
            .into_iter()
            .find(|candidate| candidate.as_str() == name)
            .ok_or_else(|| Error::Notation(format!("not a valid tie style: {name}")))
    }
}

impl fmt::Display for TieStyle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Which side of the note a mark sits on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Placement {
    /// Above the note.
    Above,
    /// Below the note.
    Below,
}

impl Placement {
    /// music21's name for the placement.
    pub fn as_str(self) -> &'static str {
        match self {
            Placement::Above => "above",
            Placement::Below => "below",
        }
    }

    /// Reads music21's name for the placement.
    pub fn from_name(name: &str) -> Result<Self> {
        match name {
            "above" => Ok(Placement::Above),
            "below" => Ok(Placement::Below),
            other => Err(Error::Notation(format!("not a valid placement: {other}"))),
        }
    }
}

impl fmt::Display for Placement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A tie joining a note to its neighbours: music21's `tie.Tie`.
///
/// A tie on the first of two notes is enough to say they sound as one; the
/// matching `Stop` on the second is what a MusicXML writer needs, not what
/// the music needs.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Tie {
    tie_type: TieType,
    style: TieStyle,
    placement: Option<Placement>,
}

impl Tie {
    /// A tie of the given type, drawn normally, with no placement of its own.
    pub fn new(tie_type: TieType) -> Self {
        Self {
            tie_type,
            style: TieStyle::default(),
            placement: None,
        }
    }

    /// Reads a tie from music21's type name, `"start"` through
    /// `"continue-let-ring"`.
    pub fn from_name(name: &str) -> Result<Self> {
        Ok(Self::new(TieType::from_name(name)?))
    }

    /// The tie type.
    pub fn tie_type(&self) -> TieType {
        self.tie_type
    }

    /// Replaces the tie type.
    pub fn set_tie_type(&mut self, tie_type: TieType) {
        self.tie_type = tie_type;
    }

    /// How the tie is drawn.
    pub fn style(&self) -> TieStyle {
        self.style
    }

    /// Sets how the tie is drawn.
    pub fn set_style(&mut self, style: TieStyle) {
        self.style = style;
    }

    /// Which side of the note the tie sits on, when it was said.
    pub fn placement(&self) -> Option<Placement> {
        self.placement
    }

    /// Sets which side of the note the tie sits on.
    pub fn set_placement(&mut self, placement: Option<Placement>) {
        self.placement = placement;
    }
}

impl Default for Tie {
    /// A starting tie, as music21's `Tie()` is.
    fn default() -> Self {
        Self::new(TieType::Start)
    }
}

impl fmt::Display for Tie {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Tie {}", self.tie_type)
    }
}

impl FromStr for Tie {
    type Err = Error;

    fn from_str(name: &str) -> Result<Self> {
        Self::from_name(name)
    }
}

/// The shape drawn for a note head: music21's `noteheadTypeNames`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Notehead {
    /// `arrow down`
    ArrowDown,
    /// `arrow up`
    ArrowUp,
    /// `back slashed`
    BackSlashed,
    /// `circle dot`
    CircleDot,
    /// `circle-x`
    CircleX,
    /// `circled`
    Circled,
    /// `cluster`
    Cluster,
    /// `cross`
    Cross,
    /// `diamond`, which is how a string harmonic is written.
    Diamond,
    /// `do`
    Do,
    /// `fa`
    Fa,
    /// `fa up`
    FaUp,
    /// `inverted triangle`
    InvertedTriangle,
    /// `la`
    La,
    /// `left triangle`
    LeftTriangle,
    /// `mi`
    Mi,
    /// `none`
    NoneShape,
    /// `normal`, the default.
    #[default]
    Normal,
    /// `other`
    Other,
    /// `re`
    Re,
    /// `rectangle`
    Rectangle,
    /// `slash`
    Slash,
    /// `slashed`
    Slashed,
    /// `so`
    So,
    /// `square`
    Square,
    /// `ti`
    Ti,
    /// `triangle`
    Triangle,
    /// `x`
    X,
}

impl Notehead {
    /// Every notehead shape, in music21's order.
    pub const ALL: [Notehead; 28] = [
        Notehead::ArrowDown,
        Notehead::ArrowUp,
        Notehead::BackSlashed,
        Notehead::CircleDot,
        Notehead::CircleX,
        Notehead::Circled,
        Notehead::Cluster,
        Notehead::Cross,
        Notehead::Diamond,
        Notehead::Do,
        Notehead::Fa,
        Notehead::FaUp,
        Notehead::InvertedTriangle,
        Notehead::La,
        Notehead::LeftTriangle,
        Notehead::Mi,
        Notehead::NoneShape,
        Notehead::Normal,
        Notehead::Other,
        Notehead::Re,
        Notehead::Rectangle,
        Notehead::Slash,
        Notehead::Slashed,
        Notehead::So,
        Notehead::Square,
        Notehead::Ti,
        Notehead::Triangle,
        Notehead::X,
    ];

    /// music21's name for the shape.
    pub fn as_str(self) -> &'static str {
        match self {
            Notehead::ArrowDown => "arrow down",
            Notehead::ArrowUp => "arrow up",
            Notehead::BackSlashed => "back slashed",
            Notehead::CircleDot => "circle dot",
            Notehead::CircleX => "circle-x",
            Notehead::Circled => "circled",
            Notehead::Cluster => "cluster",
            Notehead::Cross => "cross",
            Notehead::Diamond => "diamond",
            Notehead::Do => "do",
            Notehead::Fa => "fa",
            Notehead::FaUp => "fa up",
            Notehead::InvertedTriangle => "inverted triangle",
            Notehead::La => "la",
            Notehead::LeftTriangle => "left triangle",
            Notehead::Mi => "mi",
            Notehead::NoneShape => "none",
            Notehead::Normal => "normal",
            Notehead::Other => "other",
            Notehead::Re => "re",
            Notehead::Rectangle => "rectangle",
            Notehead::Slash => "slash",
            Notehead::Slashed => "slashed",
            Notehead::So => "so",
            Notehead::Square => "square",
            Notehead::Ti => "ti",
            Notehead::Triangle => "triangle",
            Notehead::X => "x",
        }
    }

    /// Reads music21's name for the shape.
    pub fn from_name(name: &str) -> Result<Self> {
        Self::ALL
            .into_iter()
            .find(|candidate| candidate.as_str() == name)
            .ok_or_else(|| Error::Notation(format!("not a valid notehead type name: '{name}'")))
    }
}

impl fmt::Display for Notehead {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// How one beam of a note joins its neighbours: music21's `beam.Beam`.
///
/// A beam is a horizontal line joining the flags of consecutive short notes,
/// and each note carries one for every level it is beamed at — an eighth has
/// one, a sixteenth two. Whether the line starts here, carries through, ends
/// here, or is only a stub is rhythmic grouping written down, which is why
/// this lives here beside the ties and the noteheads and not among the
/// things that need a page.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum BeamType {
    /// The beam begins on this note.
    #[default]
    Start,
    /// The beam carries through this note.
    Continue,
    /// The beam ends on this note.
    Stop,
    /// A stub reaching only part of the way, pointing left or right.
    PartialBeam,
}

impl BeamType {
    /// music21's name for the type.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Continue => "continue",
            Self::Stop => "stop",
            Self::PartialBeam => "partial",
        }
    }

    /// Reads music21's name.
    pub fn from_music21_name(name: &str) -> Option<Self> {
        match name {
            "start" => Some(Self::Start),
            "continue" => Some(Self::Continue),
            "stop" => Some(Self::Stop),
            "partial" => Some(Self::PartialBeam),
            _ => None,
        }
    }
}

impl fmt::Display for BeamType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Which way a partial beam points: music21's `direction`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum BeamDirection {
    /// Back towards the note before.
    Left,
    /// On towards the note after.
    Right,
}

impl BeamDirection {
    /// music21's name for the direction.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Right => "right",
        }
    }

    /// Reads music21's name.
    pub fn from_music21_name(name: &str) -> Option<Self> {
        match name {
            "left" => Some(Self::Left),
            "right" => Some(Self::Right),
            _ => None,
        }
    }
}

impl fmt::Display for BeamDirection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One beam at one level: music21's `beam.Beam`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Beam {
    /// What the beam does at this note. music21 leaves it unsaid on a beam
    /// that has been counted but not yet decided — which is what `fill`
    /// makes, and what a beam built with no arguments is.
    beam_type: Option<BeamType>,
    direction: Option<BeamDirection>,
    number: Option<u32>,
}

impl Beam {
    /// A beam of the given type, with a direction only a partial one needs.
    pub fn new(beam_type: impl Into<Option<BeamType>>, direction: Option<BeamDirection>) -> Self {
        Self {
            beam_type: beam_type.into(),
            direction,
            number: None,
        }
    }

    /// Whether the beam starts, carries on, ends, or is a stub — and nothing
    /// at all where that has not been decided.
    pub fn beam_type(&self) -> Option<BeamType> {
        self.beam_type
    }

    /// Says what the beam does at this note.
    pub fn set_beam_type(&mut self, beam_type: Option<BeamType>) {
        self.beam_type = beam_type;
    }

    /// Which way a stub points, and nothing for a beam that is not one.
    pub fn direction(&self) -> Option<BeamDirection> {
        self.direction
    }

    /// Points a stub the other way.
    pub fn set_direction(&mut self, direction: Option<BeamDirection>) {
        self.direction = direction;
    }

    /// Which level this beam is: the first is the one an eighth note has.
    pub fn number(&self) -> Option<u32> {
        self.number
    }

    /// Puts the beam at a level.
    pub fn set_number(&mut self, number: Option<u32>) {
        self.number = number;
    }
}

impl fmt::Display for Beam {
    /// music21's `_reprInternal`: the level, the type, and the direction of
    /// a stub.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let number = match self.number {
            Some(number) => number.to_string(),
            None => "None".to_string(),
        };
        let beam_type = match self.beam_type {
            Some(beam_type) => beam_type.to_string(),
            None => "None".to_string(),
        };
        match self.direction {
            Some(direction) => write!(f, "{number}/{beam_type}/{direction}"),
            None => write!(f, "{number}/{beam_type}"),
        }
    }
}

/// The beams of one note, one for each level it is beamed at: music21's
/// `beam.Beams`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Beams {
    beams: Vec<Beam>,
    /// Whether the beam group is drawn fanned out, for an accelerando.
    feathered: bool,
}

/// The written values that can carry a beam at all, and how many levels each
/// has: music21's `beamableDurationTypes`, an eighth through a 2048th.
const BEAMABLE: [(DurationType, u32); 9] = [
    (DurationType::Eighth, 1),
    (DurationType::Sixteenth, 2),
    (DurationType::ThirtySecond, 3),
    (DurationType::SixtyFourth, 4),
    (DurationType::HundredTwentyEighth, 5),
    (DurationType::TwoHundredFiftySixth, 6),
    (DurationType::FiveHundredTwelfth, 7),
    (DurationType::TenTwentyFourth, 8),
    (DurationType::TwentyFortyEighth, 9),
];

impl Beams {
    /// No beams at all.
    pub fn new() -> Self {
        Self::default()
    }

    /// The beams, in level order.
    pub fn beams(&self) -> &[Beam] {
        &self.beams
    }

    /// How many levels are beamed.
    pub fn len(&self) -> usize {
        self.beams.len()
    }

    /// Whether the note carries no beam at all.
    pub fn is_empty(&self) -> bool {
        self.beams.is_empty()
    }

    /// Whether the group is drawn fanned out.
    pub fn feathered(&self) -> bool {
        self.feathered
    }

    /// Draws the group fanned out, or stops doing so.
    pub fn set_feathered(&mut self, feathered: bool) {
        self.feathered = feathered;
    }

    /// The beams, to be edited in place.
    pub fn beams_mut(&mut self) -> &mut Vec<Beam> {
        &mut self.beams
    }

    /// Replaces the beams outright.
    pub fn set_beams(&mut self, beams: Vec<Beam>) {
        self.beams = beams;
    }

    /// Adds a beam at the next level down: music21's `append`.
    pub fn append(
        &mut self,
        beam_type: impl Into<Option<BeamType>>,
        direction: Option<BeamDirection>,
    ) {
        let mut beam = Beam::new(beam_type, direction);
        beam.set_number(Some(self.beams.len() as u32 + 1));
        self.beams.push(beam);
    }

    /// How many beams a written value carries, where it carries any:
    /// music21's `beamableDurationTypes` read as a count of levels.
    pub fn levels_for(duration_type: DurationType) -> Option<u32> {
        BEAMABLE
            .into_iter()
            .find(|(candidate, _)| *candidate == duration_type)
            .map(|(_, levels)| levels)
    }

    /// Gives the note as many beams as its written value has, all of the same
    /// type: music21's `fill`.
    ///
    /// A value that carries no beam — anything a quarter or longer — is an
    /// error, as it is upstream.
    pub fn fill(&mut self, duration_type: DurationType, beam_type: Option<BeamType>) -> Result<()> {
        let Some(levels) = Self::levels_for(duration_type) else {
            return Err(Error::Notation(format!(
                "cannot fill beams for a {} note",
                duration_type.music21_name()
            )));
        };
        self.fill_levels(levels, beam_type)
    }

    /// The same, counting the levels rather than naming the written value:
    /// music21 takes either, and `fill(2)` is a sixteenth.
    ///
    /// The beams are left with nothing said about what they do, unless a type
    /// is given — music21 counts the lines first and decides them after.
    pub fn fill_levels(&mut self, levels: u32, beam_type: Option<BeamType>) -> Result<()> {
        if levels == 0 || levels > BEAMABLE.len() as u32 {
            return Err(Error::Notation(format!(
                "cannot fill beams for level {levels}"
            )));
        }
        self.beams.clear();
        for _ in 0..levels {
            self.append(None, None);
        }
        if let Some(beam_type) = beam_type {
            self.set_all(beam_type, None);
        }
        Ok(())
    }

    /// Sets every beam to the same type: music21's `setAll`.
    pub fn set_all(&mut self, beam_type: BeamType, direction: Option<BeamDirection>) {
        for beam in &mut self.beams {
            beam.beam_type = Some(beam_type);
            beam.direction = direction;
        }
    }

    /// The beam at a level, counting from one: music21's `getByNumber`.
    pub fn by_number(&self, number: u32) -> Option<&Beam> {
        self.beams.iter().find(|beam| beam.number == Some(number))
    }

    /// Sets the beam at a level, which must already be there.
    pub fn set_by_number(
        &mut self,
        number: u32,
        beam_type: BeamType,
        direction: Option<BeamDirection>,
    ) -> Result<()> {
        let Some(beam) = self
            .beams
            .iter_mut()
            .find(|beam| beam.number == Some(number))
        else {
            return Err(Error::Notation(format!(
                "beam number {number} does not exist"
            )));
        };
        beam.beam_type = Some(beam_type);
        beam.direction = direction;
        Ok(())
    }

    /// The type of every beam, in level order: music21's `getTypes`.
    pub fn types(&self) -> Vec<Option<BeamType>> {
        self.beams.iter().map(Beam::beam_type).collect()
    }

    /// The level of every beam: music21's `getNumbers`.
    pub fn numbers(&self) -> Vec<Option<u32>> {
        self.beams.iter().map(Beam::number).collect()
    }
}

impl fmt::Display for Beams {
    /// music21's `_reprInternal`: every beam, slash-separated.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let written: Vec<String> = self
            .beams
            .iter()
            .map(|beam| format!("<music21.beam.Beam {beam}>"))
            .collect();
        f.write_str(&written.join("/"))
    }
}

/// A note with nothing beamable on either side of it has nothing to beam to,
/// so it carries no beam at all: music21's `removeSandwichedUnbeamables`.
///
/// The list is a run of notes, each entry the beams that note could carry or
/// nothing where it could carry none.
pub fn remove_sandwiched_unbeamables(beams: &mut [Option<Beams>]) {
    let mut previous: Option<Beams> = None;
    for index in 0..beams.len() {
        let next = beams.get(index + 1).cloned().flatten();
        if previous.is_none() && next.is_none() {
            beams[index] = None;
        }
        previous = beams[index].clone();
    }
}

/// Beams made only of stubs are no beams at all, and a stub beside a real
/// beam points towards it: music21's `sanitizePartialBeams`.
pub fn sanitize_partial_beams(beams: &mut [Option<Beams>]) {
    for entry in beams.iter_mut() {
        let Some(group) = entry else {
            continue;
        };
        let joined = group.types().into_iter().flatten().any(|beam_type| {
            matches!(
                beam_type,
                BeamType::Start | BeamType::Stop | BeamType::Continue
            )
        });
        if !joined {
            *entry = None;
            continue;
        }
        let mut after_start = false;
        let mut after_stop = false;
        for beam in group.beams_mut() {
            match beam.beam_type() {
                Some(BeamType::Start) => after_start = true,
                Some(BeamType::Stop) => after_stop = true,
                Some(BeamType::PartialBeam) => {
                    if after_start && beam.direction() == Some(BeamDirection::Left) {
                        beam.set_direction(Some(BeamDirection::Right));
                    } else if after_stop && beam.direction() == Some(BeamDirection::Right) {
                        beam.set_direction(Some(BeamDirection::Left));
                    }
                }
                _ => {}
            }
        }
    }
}

/// A stub pointing right into a stub pointing left is really one beam, and is
/// written as one: music21's `mergeConnectingPartialBeams`.
pub fn merge_connecting_partial_beams(beams: &mut [Option<Beams>]) {
    for index in 0..beams.len().saturating_sub(1) {
        let Some(numbers) = beams[index]
            .as_ref()
            .map(|group| group.numbers().into_iter().flatten().collect::<Vec<_>>())
        else {
            continue;
        };
        if beams[index + 1].is_none() {
            continue;
        }
        for number in numbers {
            let this = beams[index]
                .as_ref()
                .and_then(|group| group.by_number(number))
                .copied();
            let next = beams[index + 1]
                .as_ref()
                .and_then(|group| group.by_number(number))
                .copied();
            let (Some(this), Some(next)) = (this, next) else {
                continue;
            };
            if this.beam_type() != Some(BeamType::PartialBeam)
                || this.direction() != Some(BeamDirection::Right)
            {
                continue;
            }
            if next.beam_type() == Some(BeamType::PartialBeam)
                && next.direction() == Some(BeamDirection::Right)
            {
                continue;
            }
            // A partial pointing into a beam that carries on or ends is a
            // notation nobody can draw; music21 leaves it alone and warns.
            if matches!(
                next.beam_type(),
                Some(BeamType::Continue) | Some(BeamType::Stop)
            ) {
                continue;
            }
            set_beam(&mut beams[index], number, BeamType::Start, None);
            let merged = match next.beam_type() {
                Some(BeamType::PartialBeam) => BeamType::Stop,
                Some(BeamType::Start) => BeamType::Continue,
                other => other.unwrap_or(BeamType::Start),
            };
            set_beam(&mut beams[index + 1], number, merged, None);
        }
    }

    // And a stub pointing left after a beam that has already ended joins it.
    for index in 1..beams.len() {
        let Some(numbers) = beams[index]
            .as_ref()
            .map(|group| group.numbers().into_iter().flatten().collect::<Vec<_>>())
        else {
            continue;
        };
        if beams[index - 1].is_none() {
            continue;
        }
        for number in numbers {
            let this = beams[index]
                .as_ref()
                .and_then(|group| group.by_number(number))
                .copied();
            let previous = beams[index - 1]
                .as_ref()
                .and_then(|group| group.by_number(number))
                .copied();
            let (Some(this), Some(previous)) = (this, previous) else {
                continue;
            };
            if this.beam_type() != Some(BeamType::PartialBeam)
                || this.direction() != Some(BeamDirection::Left)
                || previous.beam_type() != Some(BeamType::Stop)
            {
                continue;
            }
            set_beam(&mut beams[index], number, BeamType::Stop, None);
            set_beam(&mut beams[index - 1], number, BeamType::Continue, None);
        }
    }
}

/// Writes one beam of one group, where the group has one at that level.
fn set_beam(
    group: &mut Option<Beams>,
    number: u32,
    beam_type: BeamType,
    direction: Option<BeamDirection>,
) {
    if let Some(group) = group {
        let _ = group.set_by_number(number, beam_type, direction);
    }
}

/// Which way the stem points: music21's `stemDirectionNames`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum StemDirection {
    /// Stems both up and down, for a divided part.
    Double,
    /// Stem down.
    Down,
    /// Written with no stem at all, as a harmonic sounding pitch is.
    NoStem,
    /// Not said, the default.
    #[default]
    Unspecified,
    /// Stem up.
    Up,
}

impl StemDirection {
    /// Every stem direction a note can hold.
    ///
    /// One shorter than music21's [`StemDirection::NAMES`]: `"none"` is an
    /// input spelling of `"noStem"` that music21's setter rewrites, so no
    /// note ever reads back as `"none"`.
    pub const ALL: [StemDirection; 5] = [
        StemDirection::Double,
        StemDirection::Down,
        StemDirection::NoStem,
        StemDirection::Unspecified,
        StemDirection::Up,
    ];

    /// music21's `stemDirectionNames`: the names the setter accepts, which
    /// includes the `"none"` spelling of `"noStem"`.
    pub const NAMES: [&'static str; 6] = ["double", "down", "noStem", "none", "unspecified", "up"];

    /// music21's name for the direction.
    pub fn as_str(self) -> &'static str {
        match self {
            StemDirection::Double => "double",
            StemDirection::Down => "down",
            StemDirection::NoStem => "noStem",
            StemDirection::Unspecified => "unspecified",
            StemDirection::Up => "up",
        }
    }

    /// Reads music21's name for the direction.
    ///
    /// `"none"` reads as [`StemDirection::NoStem`], which is what music21's
    /// setter stores for it.
    pub fn from_name(name: &str) -> Result<Self> {
        if name == "none" {
            return Ok(StemDirection::NoStem);
        }
        Self::ALL
            .into_iter()
            .find(|candidate| candidate.as_str() == name)
            .ok_or_else(|| Error::Notation(format!("not a valid stem direction name: {name}")))
    }
}

impl fmt::Display for StemDirection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Where a lyric sits in a word: music21's `Lyric.syllabic`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Syllabic {
    /// The whole word.
    Single,
    /// The first syllable of a word, hyphenated to what follows.
    Begin,
    /// A middle syllable, hyphenated on both sides.
    Middle,
    /// The last syllable of a word.
    End,
    /// A syllable that carries no hyphens, written as `text` in MusicXML.
    Composite,
}

impl Syllabic {
    /// Every syllabic position.
    pub const ALL: [Syllabic; 5] = [
        Syllabic::Single,
        Syllabic::Begin,
        Syllabic::Middle,
        Syllabic::End,
        Syllabic::Composite,
    ];

    /// music21's name for the position.
    pub fn as_str(self) -> &'static str {
        match self {
            Syllabic::Single => "single",
            Syllabic::Begin => "begin",
            Syllabic::Middle => "middle",
            Syllabic::End => "end",
            Syllabic::Composite => "composite",
        }
    }

    /// Reads music21's name for the position.
    pub fn from_name(name: &str) -> Result<Self> {
        Self::ALL
            .into_iter()
            .find(|candidate| candidate.as_str() == name)
            .ok_or_else(|| {
                // music21 quotes the value and lists what it would have
                // taken, `None` included even though it is not a position.
                Error::Notation(format!(
                    "Syllabic value '{name}' is not in note.SYLLABIC_CHOICES, namely: [None, 'begin', 'single', 'end', 'middle', 'composite']"
                ))
            })
    }
}

impl fmt::Display for Syllabic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A syllable of text sung on a note: music21's `note.Lyric`.
///
/// The hyphens in the written text say where the syllable falls in its word,
/// which is why [`Self::from_raw_text`] reads `"-ci-"` as a middle syllable
/// and [`Self::raw_text`] writes the hyphens back.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Lyric {
    text: String,
    number: IntegerType,
    syllabic: Syllabic,
    identifier: Option<String>,
    components: Vec<Lyric>,
    elision_before: String,
}

/// What music21's `Lyric.elisionBefore` starts as: a space between this
/// syllable and the one before it inside a composite lyric.
const DEFAULT_ELISION: &str = " ";

impl Lyric {
    /// A lyric on the first verse, spelled as a whole word.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            number: 1,
            syllabic: Syllabic::Single,
            identifier: None,
            components: Vec::new(),
            elision_before: DEFAULT_ELISION.to_string(),
        }
    }

    /// Reads a lyric from text whose hyphens say where it falls in its word:
    /// `"-ci-"` is a middle syllable, `"ci-"` a beginning, `"-us"` an end.
    pub fn from_raw_text(raw_text: &str) -> Self {
        let mut lyric = Self::new("");
        lyric.set_raw_text(raw_text);
        lyric
    }

    /// The syllable without its hyphens.
    ///
    /// A composite lyric has no text of its own: it reads as its components
    /// run together, each joined on by its own [`Self::elision_before`].
    pub fn text(&self) -> String {
        let Some((first, rest)) = self.components.split_first() else {
            return self.text.clone();
        };
        let mut text = first.text();
        for component in rest {
            text.push_str(&component.elision_before);
            text.push_str(&component.text());
        }
        text
    }

    /// Replaces the syllable text, leaving its position in the word alone.
    ///
    /// Setting the text of a composite lyric drops its components, which is
    /// what music21 does: the text you gave it is now the whole of it.
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.components.clear();
        self.text = text.into();
    }

    /// Whether this lyric is several lyrics sung together rather than one
    /// syllable: music21's `isComposite`.
    pub fn is_composite(&self) -> bool {
        !self.components.is_empty()
    }

    /// The lyrics this one is made of, empty unless it is composite.
    pub fn components(&self) -> &[Lyric] {
        &self.components
    }

    /// Makes this lyric the several lyrics given, run together. An empty
    /// list makes it an ordinary one again, keeping the text it had.
    pub fn set_components(&mut self, components: Vec<Lyric>) {
        self.components = components;
    }

    /// What joins this syllable to the one before it inside a composite
    /// lyric — a space by default, an underscore for an elision.
    pub fn elision_before(&self) -> &str {
        &self.elision_before
    }

    /// Sets what joins this syllable to the one before it.
    pub fn set_elision_before(&mut self, elision: impl Into<String>) {
        self.elision_before = elision.into();
    }

    /// The verse number, counting from one.
    pub fn number(&self) -> IntegerType {
        self.number
    }

    /// Sets the verse number, which must be positive as music21 requires.
    pub fn set_number(&mut self, number: IntegerType) -> Result<()> {
        if number <= 0 {
            return Err(Error::Notation(format!(
                "Number best be number {number}, not a string"
            )));
        }
        self.number = number;
        Ok(())
    }

    /// Where the syllable falls in its word. A composite lyric reads as
    /// [`Syllabic::Composite`] whatever it was set to.
    pub fn syllabic(&self) -> Syllabic {
        if self.is_composite() {
            return Syllabic::Composite;
        }
        self.syllabic
    }

    /// Sets where the syllable falls in its word.
    pub fn set_syllabic(&mut self, syllabic: Syllabic) {
        self.syllabic = syllabic;
    }

    /// The name this verse goes by, which music21 falls back to the number
    /// for when none was given.
    pub fn identifier(&self) -> String {
        self.identifier
            .clone()
            .unwrap_or_else(|| self.number.to_string())
    }

    /// The name this verse was given, and nothing when it goes by its
    /// number. music21's `identifier` answers the number itself in that
    /// case, which is a different type, so a caller that cares asks here.
    pub fn explicit_identifier(&self) -> Option<&str> {
        self.identifier.as_deref()
    }

    /// Names this verse, or clears the name so the number stands in.
    pub fn set_identifier(&mut self, identifier: Option<String>) {
        self.identifier = identifier;
    }

    /// The syllable written with the hyphens its position implies.
    ///
    /// A composite lyric takes its hyphens from the ends of the run: the
    /// first component decides whether one leads, the last whether one
    /// trails.
    pub fn raw_text(&self) -> String {
        let text = self.text();
        let (Some(first), Some(last)) = (self.components.first(), self.components.last()) else {
            return match self.syllabic {
                Syllabic::Begin => format!("{text}-"),
                Syllabic::Middle => format!("-{text}-"),
                Syllabic::End => format!("-{text}"),
                Syllabic::Single | Syllabic::Composite => text,
            };
        };
        let leading = matches!(first.syllabic(), Syllabic::Middle | Syllabic::End);
        let trailing = matches!(last.syllabic(), Syllabic::Begin | Syllabic::Middle);
        format!(
            "{}{text}{}",
            if leading { "-" } else { "" },
            if trailing { "-" } else { "" }
        )
    }

    /// Reads the syllable and its position out of hyphenated text. Like
    /// [`Self::set_text`], this drops any components.
    pub fn set_raw_text(&mut self, raw_text: &str) {
        self.components.clear();
        let starts = raw_text.starts_with('-');
        let ends = raw_text.ends_with('-') && raw_text.len() > 1;
        let trimmed = raw_text.strip_prefix('-').unwrap_or(raw_text).to_string();
        let trimmed = if ends {
            trimmed.strip_suffix('-').unwrap_or(&trimmed).to_string()
        } else {
            trimmed
        };
        self.syllabic = match (starts, ends) {
            (true, true) => Syllabic::Middle,
            (true, false) => Syllabic::End,
            (false, true) => Syllabic::Begin,
            (false, false) => Syllabic::Single,
        };
        self.text = trimmed;
    }
}

impl fmt::Display for Lyric {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.text())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn beams_fill_one_level_for_each_flag() {
        // music21's own example: a sixteenth is beamed at two levels.
        let mut beams = Beams::new();
        beams
            .fill(DurationType::Sixteenth, Some(BeamType::Start))
            .unwrap();
        assert_eq!(beams.len(), 2);
        assert_eq!(
            beams.types(),
            [Some(BeamType::Start), Some(BeamType::Start)]
        );
        assert_eq!(beams.numbers(), [Some(1), Some(2)]);
        assert_eq!(
            beams.to_string(),
            "<music21.beam.Beam 1/start>/<music21.beam.Beam 2/start>"
        );

        beams.set_all(BeamType::Stop, None);
        assert_eq!(beams.types(), [Some(BeamType::Stop), Some(BeamType::Stop)]);
        assert_eq!(
            beams.by_number(1).and_then(Beam::beam_type),
            Some(BeamType::Stop)
        );

        // Counted but not decided: `fill` alone says how many lines there
        // are and leaves what each does unsaid, as music21 does.
        let mut counted = Beams::new();
        counted.fill(DurationType::Sixteenth, None).unwrap();
        assert_eq!(counted.types(), [None, None]);
        assert_eq!(
            counted.to_string(),
            "<music21.beam.Beam 1/None>/<music21.beam.Beam 2/None>"
        );
        assert!(Beams::new().fill_levels(12, None).is_err());
        assert!(beams.set_by_number(3, BeamType::Start, None).is_err());

        // A quarter carries no beam at all, and music21 refuses to fill one.
        assert!(Beams::new().fill(DurationType::Quarter, None).is_err());

        // A stub says which way it points.
        let mut stub = Beams::new();
        stub.append(BeamType::PartialBeam, Some(BeamDirection::Left));
        assert_eq!(stub.to_string(), "<music21.beam.Beam 1/partial/left>");
    }
    use super::*;

    #[test]
    fn tie_types_round_trip_and_reject_others() {
        for tie_type in TieType::ALL {
            assert_eq!(TieType::from_name(tie_type.as_str()).unwrap(), tie_type);
        }
        assert_eq!(Tie::default().tie_type(), TieType::Start);
        assert_eq!(Tie::from_name("stop").unwrap().to_string(), "Tie stop");
        let error = TieType::from_name("hello").unwrap_err().to_string();
        assert!(error.contains("Type must be one of"), "{error}");
        assert!(error.ends_with("not hello"), "{error}");
    }

    #[test]
    fn tie_carries_style_and_placement() {
        let mut tie = Tie::new(TieType::Continue);
        assert_eq!(tie.style(), TieStyle::Normal);
        assert_eq!(tie.placement(), None);
        tie.set_style(TieStyle::Dashed);
        tie.set_placement(Some(Placement::Above));
        assert_eq!(tie.style().as_str(), "dashed");
        assert_eq!(tie.placement().map(Placement::as_str), Some("above"));
        assert!(TieStyle::from_name("wavy").is_err());
        assert!(Placement::from_name("sideways").is_err());
    }

    #[test]
    fn notehead_and_stem_names_round_trip() {
        for notehead in Notehead::ALL {
            assert_eq!(Notehead::from_name(notehead.as_str()).unwrap(), notehead);
        }
        for direction in StemDirection::ALL {
            assert_eq!(
                StemDirection::from_name(direction.as_str()).unwrap(),
                direction
            );
        }
        assert_eq!(Notehead::default(), Notehead::Normal);
        // "none" is an input spelling of "noStem", never a state of its own.
        assert_eq!(
            StemDirection::from_name("none").unwrap(),
            StemDirection::NoStem
        );
        assert_eq!(StemDirection::NAMES.len(), StemDirection::ALL.len() + 1);
        assert_eq!(StemDirection::default(), StemDirection::Unspecified);
        assert_eq!(Notehead::Diamond.to_string(), "diamond");
        assert_eq!(StemDirection::NoStem.to_string(), "noStem");
        assert!(Notehead::from_name("blah").is_err());
        assert!(StemDirection::from_name("sideways").is_err());
    }

    #[test]
    fn lyric_hyphens_say_where_the_syllable_falls() {
        let cases = [
            ("hello", Syllabic::Single, "hello"),
            ("dic-", Syllabic::Begin, "dic"),
            ("-ci-", Syllabic::Middle, "ci"),
            ("-us", Syllabic::End, "us"),
        ];
        for (raw, syllabic, text) in cases {
            let lyric = Lyric::from_raw_text(raw);
            assert_eq!(lyric.syllabic(), syllabic, "{raw}");
            assert_eq!(lyric.text(), text, "{raw}");
            assert_eq!(lyric.raw_text(), raw, "{raw}");
        }
        let mut lyric = Lyric::new("shine");
        assert_eq!(lyric.number(), 1);
        assert_eq!(lyric.identifier(), "1");
        lyric.set_number(3).unwrap();
        assert_eq!(lyric.identifier(), "3");
        lyric.set_identifier(Some("chorus".to_string()));
        assert_eq!(lyric.identifier(), "chorus");
        assert!(lyric.set_number(0).is_err());
        assert_eq!(lyric.to_string(), "shine");
        assert_eq!(Syllabic::from_name("middle").unwrap(), Syllabic::Middle);
        assert!(Syllabic::from_name("half").is_err());
    }

    #[test]
    fn a_composite_lyric_reads_as_its_components_run_together() {
        // music21's own example: "bianco" sung as "co" then "e".
        let mut co = Lyric::new("co");
        co.set_syllabic(Syllabic::End);
        let mut e = Lyric::new("e");
        e.set_syllabic(Syllabic::Single);

        let mut bianco = Lyric::new("");
        assert!(!bianco.is_composite());
        bianco.set_components(vec![co, e]);
        assert!(bianco.is_composite());
        assert_eq!(bianco.text(), "co e");
        assert_eq!(bianco.syllabic(), Syllabic::Composite);
        // The first component leads with a hyphen; the last ends the word.
        assert_eq!(bianco.raw_text(), "-co e");

        // An elision joins the two rather than a space, and a middle
        // syllable at the end leaves the word open.
        let mut components = bianco.components().to_vec();
        components[1].set_elision_before("_");
        components[1].set_syllabic(Syllabic::Middle);
        bianco.set_components(components);
        assert_eq!(bianco.text(), "co_e");
        assert_eq!(bianco.raw_text(), "-co_e-");

        // Setting the text outright is music21's way of un-compositing.
        bianco.set_text("bianco");
        assert!(!bianco.is_composite());
        assert_eq!(bianco.text(), "bianco");
        assert_eq!(bianco.syllabic(), Syllabic::Single);
    }

    #[test]
    fn an_identifier_is_only_there_when_it_was_named() {
        let mut lyric = Lyric::new("shine");
        assert_eq!(lyric.explicit_identifier(), None);
        assert_eq!(lyric.identifier(), "1");
        lyric.set_identifier(Some("chorus".to_string()));
        assert_eq!(lyric.explicit_identifier(), Some("chorus"));
    }

    #[test]
    fn a_lone_hyphen_is_its_own_syllable() {
        // "-" is a beginning-of-word marker in music21, not an end marker
        // with empty text, so the trailing hyphen is only stripped when
        // something else is there to strip it from.
        let lyric = Lyric::from_raw_text("-");
        assert_eq!(lyric.syllabic(), Syllabic::End);
        assert_eq!(lyric.text(), "");
    }
}
