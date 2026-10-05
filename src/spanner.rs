//! Spanners: marks that join several notes, music21's `spanner` module.
//!
//! music21 keeps a spanner as an object holding the very notes it joins. A
//! stream here owns what it holds, so a spanner names its notes by where they
//! stand instead: by their positions in [`Stream::leaves`] of the stream the
//! spanner belongs to, first to last.
//!
//! [`Stream::leaves`]: crate::Stream::leaves

use crate::defaults::FloatType;
use crate::display::Drawn;
use crate::notation::Placement;

/// Which of music21's spanner classes a spanner is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum SpannerKind {
    /// A curve joining notes played in one breath or bow: music21's `Slur`.
    Slur,
    /// A hairpin opening out, growing louder: music21's `Crescendo`.
    Crescendo,
    /// A hairpin closing, growing softer: music21's `Diminuendo`.
    Diminuendo,
    /// An octave line, `8va` or `15mb`: music21's `Ottava`.
    Ottava,
    /// A bracket or line over notes, as a *ligatura* or a passage to be
    /// played together: music21's `Line`.
    Line,
    /// How long a pedal is held: music21's `PedalMark`.
    PedalMark,
    /// The wavy line carrying a trill on past its note: music21's
    /// `TrillExtension`.
    TrillExtension,
    /// One arpeggio across several chords, as across both hands' staves:
    /// music21's `ArpeggioMarkSpanner`.
    ArpeggioMark,
    /// A glissando or a slide from one note to the next: music21's
    /// `Glissando`.
    Glissando,
    /// A tremolo alternating between two notes: music21's `TremoloSpanner`.
    TremoloSpanner,
    /// Notes sung to one syllable as one figure, as chant writes them:
    /// music21's volpiano `Neume`.
    Neume,
}

impl SpannerKind {
    /// music21's class name for the kind.
    pub fn class_name(self) -> &'static str {
        match self {
            Self::Slur => "Slur",
            Self::Crescendo => "Crescendo",
            Self::Diminuendo => "Diminuendo",
            Self::Ottava => "Ottava",
            Self::Line => "Line",
            Self::PedalMark => "PedalMark",
            Self::ArpeggioMark => "ArpeggioMarkSpanner",
            Self::TrillExtension => "TrillExtension",
            Self::Glissando => "Glissando",
            Self::TremoloSpanner => "TremoloSpanner",
            Self::Neume => "Neume",
        }
    }

    /// The class spanners of this kind are counted with where music21 numbers
    /// them for a score: a crescendo and a diminuendo are both hairpins,
    /// `DynamicWedge`, and share their numbers.
    pub fn numbering_class(self) -> &'static str {
        match self {
            Self::Crescendo | Self::Diminuendo => "DynamicWedge",
            other => other.class_name(),
        }
    }

    /// Whether this is a hairpin: music21's `DynamicWedge`.
    pub fn is_wedge(self) -> bool {
        matches!(self, Self::Crescendo | Self::Diminuendo)
    }
}

/// A mark joining several notes: music21's `Spanner`.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[must_use]
pub struct Spanner {
    kind: SpannerKind,
    /// Each element joined, by position, or nothing for one that stands in
    /// no stream at all -- which music21 allows, and still counts.
    spanned: Vec<Option<usize>>,
    #[cfg_attr(feature = "serde", serde(default))]
    placement: Drawn<Option<Placement>>,
    #[cfg_attr(feature = "serde", serde(default))]
    line_type: Drawn<Option<String>>,
    #[cfg_attr(feature = "serde", serde(default))]
    spread: Drawn<Option<FloatType>>,
    #[cfg_attr(feature = "serde", serde(default))]
    shift: Option<OctaveShift>,
    #[cfg_attr(feature = "serde", serde(default))]
    ends: Drawn<LineEnds>,
    #[cfg_attr(feature = "serde", serde(default))]
    pedal: Option<Pedal>,
    #[cfg_attr(feature = "serde", serde(default))]
    arpeggio: crate::expressions::ArpeggioType,
    #[cfg_attr(feature = "serde", serde(default))]
    glissando: Option<Glissando>,
    #[cfg_attr(feature = "serde", serde(default))]
    marks: Option<u8>,
    #[cfg_attr(feature = "serde", serde(default))]
    offset: Drawn<FloatType>,
}

/// How a glissando is played: music21's `Glissando.slideType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum SlideType {
    /// Through every semitone between, which is how music21's starts out.
    #[default]
    Chromatic,
    /// With no steps at all, a slide or a smear.
    Continuous,
    /// Through the notes of the scale, as a harp plays one.
    Diatonic,
    /// Over the white keys.
    White,
    /// Over the black keys.
    Black,
}

impl SlideType {
    /// music21's name for it: `chromatic`, `continuous`, `diatonic`,
    /// `white` or `black`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Chromatic => "chromatic",
            Self::Continuous => "continuous",
            Self::Diatonic => "diatonic",
            Self::White => "white",
            Self::Black => "black",
        }
    }

    /// Reads music21's name for a slide, whatever its case.
    ///
    /// # Errors
    ///
    /// A name that is none of the five, as music21's `SpannerException`.
    pub fn from_name(name: &str) -> crate::error::Result<Self> {
        let lower = name.to_lowercase();
        [
            Self::Chromatic,
            Self::Continuous,
            Self::Diatonic,
            Self::White,
            Self::Black,
        ]
        .into_iter()
        .find(|kind| kind.as_str() == lower)
        .ok_or_else(|| crate::error::Error::Value(format!("not a valid value: {name}")))
    }
}

/// What a glissando says beside the notes it joins: how it is played and
/// the words written along it, music21's `slideType` and `label`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Glissando {
    /// How it is played.
    pub slide_type: SlideType,
    /// The words along the line, *gliss.*, where it has any.
    pub label: Option<String>,
}

/// Which of music21's pedal objects a [`PedalObject`] is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum PedalObjectKind {
    /// The pedal let up and put down again at once: music21's
    /// `PedalBounce`.
    Bounce,
    /// Where the pedal line stops being drawn while the pedalling goes on,
    /// usually beside *simile*: music21's `PedalGapStart`.
    GapStart,
    /// Where the pedal line is drawn again: music21's `PedalGapEnd`.
    GapEnd,
}

impl PedalObjectKind {
    /// music21's class name for it.
    pub fn class_name(self) -> &'static str {
        match self {
            Self::Bounce => "PedalBounce",
            Self::GapStart => "PedalGapStart",
            Self::GapEnd => "PedalGapEnd",
        }
    }

    /// Reads music21's class name for one.
    pub fn from_class_name(name: &str) -> Option<Self> {
        [Self::Bounce, Self::GapStart, Self::GapEnd]
            .into_iter()
            .find(|kind| kind.class_name() == name)
    }
}

/// A moment inside a held pedal -- a bounce, or a gap in its line -- which
/// stands in the stream at its own offset and is joined by the pedal mark
/// it belongs to: music21's `PedalObject`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[must_use]
pub struct PedalObject {
    kind: PedalObjectKind,
    #[cfg_attr(feature = "serde", serde(default))]
    placement: Drawn<Option<Placement>>,
}

impl PedalObject {
    /// A pedal object of a kind.
    pub fn new(kind: PedalObjectKind) -> Self {
        Self {
            kind,
            placement: Drawn(None),
        }
    }

    /// Which kind it is.
    pub fn kind(&self) -> PedalObjectKind {
        self.kind
    }

    /// Which side of the staff it is drawn on, where the score says.
    pub fn placement(&self) -> Option<Placement> {
        self.placement.0
    }

    /// Says which side of the staff it is drawn on.
    pub fn set_placement(&mut self, placement: Option<Placement>) {
        self.placement.0 = placement;
    }
}

/// How a line's ends are drawn: music21's `startTick`, `endTick`,
/// `startHeight` and `endHeight`.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LineEnds {
    /// The tick at the start.
    pub start: LineEnd,
    /// The tick at the end.
    pub end: LineEnd,
    /// How long the tick at the start is, in tenths of a staff space.
    pub start_height: Option<FloatType>,
    /// How long the tick at the end is, in tenths of a staff space.
    pub end_height: Option<FloatType>,
}

/// The tick at one end of a line.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum LineEnd {
    /// A tick up.
    Up,
    /// A tick down, which is how music21's lines start out.
    #[default]
    Down,
    /// An arrowhead.
    Arrow,
    /// A tick both ways.
    Both,
    /// No tick at all.
    None,
}

impl LineEnd {
    /// music21's name for it, which is MusicXML's too: `up`, `down`,
    /// `arrow`, `both` or `none`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Up => "up",
            Self::Down => "down",
            Self::Arrow => "arrow",
            Self::Both => "both",
            Self::None => "none",
        }
    }

    /// Reads music21's name for a tick, whatever its case.
    pub fn from_name(name: &str) -> crate::error::Result<Self> {
        Ok(match name.to_lowercase().as_str() {
            "up" => Self::Up,
            "down" => Self::Down,
            "arrow" => Self::Arrow,
            "both" => Self::Both,
            "none" => Self::None,
            other => {
                return Err(crate::error::Error::Value(format!(
                    "not a valid value: {other}"
                )));
            }
        })
    }
}

/// Which pedal a pedal mark holds down: music21's `PedalType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum PedalType {
    /// The sustaining pedal.
    Sustain,
    /// The sostenuto pedal, which holds only the notes already down.
    Sostenuto,
    /// The soft pedal.
    Soft,
    /// A practice piano's silent pedal.
    Silent,
}

/// How a pedal mark is drawn: music21's `PedalForm`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum PedalForm {
    /// A bracket under the notes held.
    Line,
    /// *Ped.* where it goes down and a star where it comes up.
    Symbol,
    /// *Ped.* where it goes down and nothing where it comes up.
    SymbolAlt,
    /// *Ped.* where it goes down and a line from there.
    SymbolLine,
}

/// A pedal mark's pedal and how it is drawn: music21's `pedalType`,
/// `pedalForm` and `abbreviated`, the first two of which a score need not
/// say.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Pedal {
    /// Which pedal is held.
    pub pedal_type: Option<PedalType>,
    /// How the mark is drawn.
    pub form: Option<PedalForm>,
    /// Whether the symbol is shortened, *P.* for *Ped.*
    pub abbreviated: bool,
}

impl PedalType {
    /// music21's name for it: `sustain`, `sostenuto`, `soft` or `silent`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Sustain => "sustain",
            Self::Sostenuto => "sostenuto",
            Self::Soft => "soft",
            Self::Silent => "silent",
        }
    }

    /// Reads music21's name for a pedal.
    pub fn from_name(name: &str) -> Option<Self> {
        [Self::Sustain, Self::Sostenuto, Self::Soft, Self::Silent]
            .into_iter()
            .find(|kind| kind.as_str() == name)
    }
}

impl PedalForm {
    /// music21's name for it: `line`, `symbol`, `symbolalt` or
    /// `symbolline`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Line => "line",
            Self::Symbol => "symbol",
            Self::SymbolAlt => "symbolalt",
            Self::SymbolLine => "symbolline",
        }
    }

    /// Reads music21's name for a form.
    pub fn from_name(name: &str) -> Option<Self> {
        [Self::Line, Self::Symbol, Self::SymbolAlt, Self::SymbolLine]
            .into_iter()
            .find(|form| form.as_str() == name)
    }

    /// Whether a line is drawn from where the pedal goes down.
    pub fn has_line(self) -> bool {
        matches!(self, Self::Line | Self::SymbolLine)
    }
}

/// How far and which way an octave line moves the notes under it:
/// music21's `Ottava.type`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct OctaveShift {
    size: u8,
    up: bool,
    transposing: bool,
}

impl OctaveShift {
    /// A shift by music21's name for it: `8va`, `8vb`, `15ma`, `15mb`,
    /// `22da` or `22db`. `transposing` says whether the notes under it are
    /// written where they sound, as music21's `transposing` says.
    pub fn from_name(name: &str, transposing: bool) -> crate::error::Result<Self> {
        let (size, up) = match name.to_lowercase().as_str() {
            "8va" => (8, true),
            "8vb" => (8, false),
            "15ma" => (15, true),
            "15mb" => (15, false),
            "22da" => (22, true),
            "22db" => (22, false),
            other => {
                return Err(crate::error::Error::Value(format!(
                    "cannot create Ottava of type: {other}"
                )));
            }
        };
        Ok(Self {
            size,
            up,
            transposing,
        })
    }

    /// music21's name for the shift, `8va`.
    pub fn name(&self) -> &'static str {
        match (self.size, self.up) {
            (8, true) => "8va",
            (8, false) => "8vb",
            (15, true) => "15ma",
            (15, false) => "15mb",
            (_, true) => "22da",
            (_, false) => "22db",
        }
    }

    /// 8, 15 or 22: music21's `shiftMagnitude`.
    pub fn magnitude(&self) -> u8 {
        self.size
    }

    /// Whether the notes sound higher than written: an `8va` does.
    pub fn up(&self) -> bool {
        self.up
    }

    /// Whether the notes under the line are written where they sound:
    /// music21's `transposing`.
    pub fn transposing(&self) -> bool {
        self.transposing
    }
}

impl Spanner {
    /// A spanner of a kind joining the elements at these positions.
    pub fn new(kind: SpannerKind, spanned: Vec<usize>) -> Self {
        Self::with_unplaced(kind, spanned.into_iter().map(Some).collect())
    }

    /// A spanner of a kind joining elements some of which stand nowhere: a
    /// score read from a file may leave a spanner holding a note no stream
    /// holds. It still counts that note among its own, so the ends and the
    /// length are those of the whole list.
    pub fn with_unplaced(kind: SpannerKind, spanned: Vec<Option<usize>>) -> Self {
        Self {
            kind,
            spanned,
            placement: Drawn(None),
            line_type: Drawn(None),
            spread: Drawn(None),
            shift: None,
            ends: Drawn(LineEnds::default()),
            pedal: None,
            arpeggio: crate::expressions::ArpeggioType::Normal,
            glissando: None,
            marks: None,
            offset: Drawn(0.0),
        }
    }

    /// A glissando over the elements at these positions, drawn as a wavy
    /// line and played chromatically, as music21's starts out.
    pub fn glissando(spanned: Vec<usize>) -> Self {
        let mut glissando = Self::new(SpannerKind::Glissando, spanned);
        glissando.line_type = Drawn(Some("wavy".to_string()));
        glissando.glissando = Some(Glissando::default());
        glissando
    }

    /// How a glissando is played and what is written along it.
    pub fn glissando_details(&self) -> Option<&Glissando> {
        self.glissando.as_ref()
    }

    /// Says how a glissando is played and what is written along it.
    pub fn set_glissando_details(&mut self, glissando: Option<Glissando>) {
        self.glissando = glissando;
    }

    /// A tremolo alternating between the elements at these positions, with
    /// so many strokes through the stems.
    ///
    /// # Errors
    ///
    /// More than eight strokes, as music21's `TremoloException`.
    pub fn tremolo(marks: u8, spanned: Vec<usize>) -> crate::error::Result<Self> {
        let mut tremolo = Self::new(SpannerKind::TremoloSpanner, spanned);
        tremolo.set_number_of_marks(Some(marks))?;
        Ok(tremolo)
    }

    /// How many strokes a tremolo between notes is written with: music21's
    /// `numberOfMarks`.
    pub fn number_of_marks(&self) -> Option<u8> {
        self.marks
    }

    /// Says how many strokes a tremolo between notes is written with.
    ///
    /// # Errors
    ///
    /// More than eight, as music21's `TremoloException`.
    pub fn set_number_of_marks(&mut self, marks: Option<u8>) -> crate::error::Result<()> {
        if marks.is_some_and(|marks| marks > 8) {
            return Err(crate::error::Error::Value(
                "Number of marks must be a number from 0 to 8".to_string(),
            ));
        }
        self.marks = marks;
        Ok(())
    }

    /// Where the spanner itself stands in the stream holding it: nought for
    /// one a score is read with. A pedal mark drawn as a sign and a line
    /// says where its line resumes by it.
    pub fn offset(&self) -> FloatType {
        self.offset.0
    }

    /// Says where the spanner itself stands in the stream holding it.
    pub fn set_offset(&mut self, offset: FloatType) {
        self.offset.0 = offset;
    }

    /// One arpeggio across the chords at these positions.
    pub fn arpeggio_mark(arpeggio: crate::expressions::ArpeggioType, spanned: Vec<usize>) -> Self {
        let mut mark = Self::new(SpannerKind::ArpeggioMark, spanned);
        mark.arpeggio = arpeggio;
        mark
    }

    /// Which way an arpeggio across several chords is spread.
    pub fn arpeggio(&self) -> crate::expressions::ArpeggioType {
        self.arpeggio
    }

    /// Says which way an arpeggio across several chords is spread.
    pub fn set_arpeggio(&mut self, arpeggio: crate::expressions::ArpeggioType) {
        self.arpeggio = arpeggio;
    }

    /// A line over the elements at these positions, solid with a tick down
    /// at each end, as music21's starts out.
    pub fn line(spanned: Vec<usize>) -> Self {
        let mut line = Self::new(SpannerKind::Line, spanned);
        line.line_type = Drawn(Some("solid".to_string()));
        line
    }

    /// How a line's ends are drawn.
    pub fn ends(&self) -> LineEnds {
        self.ends.0
    }

    /// Says how a line's ends are drawn.
    pub fn set_ends(&mut self, ends: LineEnds) {
        self.ends.0 = ends;
    }

    /// A pedal held under the elements at these positions.
    pub fn pedal_mark(pedal: Pedal, spanned: Vec<usize>) -> Self {
        let mut mark = Self::new(SpannerKind::PedalMark, spanned);
        mark.pedal = Some(pedal);
        mark
    }

    /// Which pedal a pedal mark holds and how it is drawn.
    pub fn pedal(&self) -> Option<Pedal> {
        self.pedal
    }

    /// Says which pedal a pedal mark holds and how it is drawn.
    pub fn set_pedal(&mut self, pedal: Option<Pedal>) {
        self.pedal = pedal;
    }

    /// An octave line over the elements at these positions, drawn above them
    /// as music21's starts out.
    pub fn ottava(shift: OctaveShift, spanned: Vec<usize>) -> Self {
        let mut ottava = Self::new(SpannerKind::Ottava, spanned);
        ottava.placement = Drawn(Some(Placement::Above));
        ottava.shift = Some(shift);
        ottava
    }

    /// How far and which way an octave line moves its notes.
    pub fn shift(&self) -> Option<OctaveShift> {
        self.shift
    }

    /// Says how far and which way an octave line moves its notes.
    pub fn set_shift(&mut self, shift: Option<OctaveShift>) {
        self.shift = shift;
    }

    /// A slur over the elements at these positions.
    pub fn slur(spanned: Vec<usize>) -> Self {
        Self::new(SpannerKind::Slur, spanned)
    }

    /// A hairpin over the elements at these positions, drawn below them and
    /// opening fifteen tenths wide, as music21's starts out.
    pub fn wedge(kind: SpannerKind, spanned: Vec<usize>) -> Self {
        let mut wedge = Self::new(kind, spanned);
        wedge.placement = Drawn(Some(Placement::Below));
        wedge.spread = Drawn(Some(15.0));
        wedge
    }

    /// How wide a hairpin opens, in tenths of a staff space: music21's
    /// `spread`.
    pub fn spread(&self) -> Option<FloatType> {
        self.spread.0
    }

    /// Says how wide a hairpin opens.
    pub fn set_spread(&mut self, spread: Option<FloatType>) {
        self.spread.0 = spread;
    }

    /// Which kind of spanner this is.
    pub fn kind(&self) -> SpannerKind {
        self.kind
    }

    /// The positions of the elements it joins, first to last, nothing for
    /// one that stands in no stream.
    pub fn spanned(&self) -> &[Option<usize>] {
        &self.spanned
    }

    /// The positions of the elements it joins, to be moved where an edit
    /// has moved them.
    pub(crate) fn spanned_mut(&mut self) -> &mut Vec<Option<usize>> {
        &mut self.spanned
    }

    /// Moves each place it names to where `moved` says that element now
    /// stands, indexed by the old place: nothing for one no longer held.
    pub(crate) fn move_places(&mut self, moved: &[Option<usize>]) {
        for place in &mut self.spanned {
            *place = place.and_then(|old| moved.get(old).copied().flatten());
        }
    }

    /// Joins an element once where moving its places has made it join one
    /// twice, keeping the first.
    pub(crate) fn drop_repeated_places(&mut self) {
        let mut seen: Vec<usize> = Vec::new();
        self.spanned.retain(|place| match place {
            Some(place) if seen.contains(place) => false,
            Some(place) => {
                seen.push(*place);
                true
            }
            None => true,
        });
    }

    /// How many elements it joins, those standing nowhere included.
    pub fn len(&self) -> usize {
        self.spanned.len()
    }

    /// Whether it joins nothing at all.
    pub fn is_empty(&self) -> bool {
        self.spanned.is_empty()
    }

    /// Whether the element at a position is the first it joins: music21's
    /// `isFirst`.
    pub fn is_first(&self, position: usize) -> bool {
        self.spanned.first() == Some(&Some(position))
    }

    /// Whether the element at a position is the last it joins: music21's
    /// `isLast`.
    pub fn is_last(&self, position: usize) -> bool {
        self.spanned.last() == Some(&Some(position))
    }

    /// Which side of the notes it is drawn on, where the score says.
    pub fn placement(&self) -> Option<Placement> {
        self.placement.0
    }

    /// Says which side of the notes it is drawn on.
    pub fn set_placement(&mut self, placement: Option<Placement>) {
        self.placement.0 = placement;
    }

    /// How its line is drawn where that is not solid, `dashed` or `dotted`:
    /// music21's `style.lineType`.
    pub fn line_type(&self) -> Option<&str> {
        self.line_type.0.as_deref()
    }

    /// Says how its line is drawn.
    pub fn set_line_type(&mut self, line_type: Option<String>) {
        self.line_type.0 = line_type;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_spanner_knows_its_ends() {
        let slur = Spanner::slur(vec![3, 4, 5]);
        assert!(slur.is_first(3));
        assert!(slur.is_last(5));
        assert!(!slur.is_first(4) && !slur.is_last(4));
        assert_eq!(slur.kind().class_name(), "Slur");
    }
}
