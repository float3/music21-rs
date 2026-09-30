//! Words and signs telling a player where to go next: music21's repeat
//! expressions, `Segno`, `Coda`, `Fine`, *Da Capo* and the rest.
//!
//! The barline repeats and the endings are [`crate::bar`]'s; these are the
//! marks written above the staff.

use crate::display::Drawn;
use crate::notation::Placement;

/// Which of music21's repeat expressions a mark is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum RepeatExpressionKind {
    /// The place a *to Coda* jumps to: music21's `Coda`.
    Coda,
    /// The sign a *Dal Segno* goes back to: music21's `Segno`.
    Segno,
    /// Where the piece ends after a jump back: music21's `Fine`.
    Fine,
    /// Back to the start: music21's `DaCapo`.
    DaCapo,
    /// Back to the start and on to *Fine*: music21's `DaCapoAlFine`.
    DaCapoAlFine,
    /// Back to the start and on to the coda: music21's `DaCapoAlCoda`.
    DaCapoAlCoda,
    /// On to the sign: music21's `AlSegno`.
    AlSegno,
    /// Back to the sign: music21's `DalSegno`.
    DalSegno,
    /// Back to the sign and on to *Fine*: music21's `DalSegnoAlFine`.
    DalSegnoAlFine,
    /// Back to the sign and on to the coda: music21's `DalSegnoAlCoda`.
    DalSegnoAlCoda,
}

impl RepeatExpressionKind {
    /// Every kind, in the order music21's module defines them.
    pub const ALL: [Self; 10] = [
        Self::Coda,
        Self::Segno,
        Self::Fine,
        Self::DaCapo,
        Self::DaCapoAlFine,
        Self::DaCapoAlCoda,
        Self::AlSegno,
        Self::DalSegno,
        Self::DalSegnoAlFine,
        Self::DalSegnoAlCoda,
    ];

    /// music21's class name for the kind.
    pub fn class_name(self) -> &'static str {
        match self {
            Self::Coda => "Coda",
            Self::Segno => "Segno",
            Self::Fine => "Fine",
            Self::DaCapo => "DaCapo",
            Self::DaCapoAlFine => "DaCapoAlFine",
            Self::DaCapoAlCoda => "DaCapoAlCoda",
            Self::AlSegno => "AlSegno",
            Self::DalSegno => "DalSegno",
            Self::DalSegnoAlFine => "DalSegnoAlFine",
            Self::DalSegnoAlCoda => "DalSegnoAlCoda",
        }
    }

    /// The kind music21 names by this class name.
    pub fn from_class_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.class_name() == name)
    }

    /// The ways the mark may be written, the first being how it is written
    /// when nothing says otherwise: music21's `_textAlternatives`.
    pub fn text_alternatives(self) -> &'static [&'static str] {
        match self {
            Self::Coda => &["Coda", "to Coda", "al Coda"],
            Self::Segno => &["Segno"],
            Self::Fine => &["fine"],
            Self::DaCapo => &["Da Capo", "D.C."],
            Self::DaCapoAlFine => &["Da Capo al fine", "D.C. al fine"],
            Self::DaCapoAlCoda => &["Da Capo al Coda", "D.C. al Coda"],
            Self::AlSegno => &["al Segno"],
            Self::DalSegno => &["Dal Segno", "D.S."],
            Self::DalSegnoAlFine => &["Dal Segno al fine", "D.S. al fine"],
            Self::DalSegnoAlCoda => &["Dal Segno al Coda", "D.S. al Coda"],
        }
    }

    /// Whether the mark is a place to jump to rather than an instruction to
    /// jump: music21's `RepeatExpressionMarker` against
    /// `RepeatExpressionCommand`.
    pub fn is_marker(self) -> bool {
        matches!(self, Self::Coda | Self::Segno | Self::Fine)
    }

    /// How the words are justified: an instruction to jump stands at the
    /// end of the bar it acts on, and so does *Fine*.
    pub fn justify(self) -> &'static str {
        match self {
            Self::Coda | Self::Segno => "center",
            _ => "right",
        }
    }

    /// Whether the mark has a sign of its own: only the coda and the segno
    /// do.
    pub fn has_symbol(self) -> bool {
        matches!(self, Self::Coda | Self::Segno)
    }
}

/// A word or sign telling a player where to go next: music21's
/// `RepeatExpression`.
///
/// ```
/// use music21_rs::repeat::{RepeatExpression, RepeatExpressionKind};
///
/// let fine = RepeatExpression::new(RepeatExpressionKind::Fine);
/// assert_eq!(fine.text(), "fine");
/// assert!(!fine.use_symbol());
///
/// let coda = RepeatExpression::with_text(RepeatExpressionKind::Coda, "to Coda");
/// assert_eq!(coda.text(), "to Coda");
/// assert!(!coda.use_symbol());
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[must_use]
pub struct RepeatExpression {
    kind: RepeatExpressionKind,
    text: String,
    use_symbol: bool,
    #[cfg_attr(feature = "serde", serde(default))]
    placement: Drawn<Option<Placement>>,
}

impl RepeatExpression {
    /// A mark of a kind, written as music21 writes it when nothing says
    /// otherwise: a coda and a segno as their signs, everything else in
    /// words.
    pub fn new(kind: RepeatExpressionKind) -> Self {
        Self {
            kind,
            text: kind.text_alternatives()[0].to_string(),
            use_symbol: kind.has_symbol(),
            placement: Drawn(None),
        }
    }

    /// A mark of a kind written in these words, where they are one of its
    /// ways of being written, and as it is written by default otherwise:
    /// music21's `text` argument. A coda given words is written in them
    /// rather than as its sign. A segno and *fine* are always written their
    /// own way.
    pub fn with_text(kind: RepeatExpressionKind, text: &str) -> Self {
        let mut mark = Self::new(kind);
        let takes_text = !matches!(
            kind,
            RepeatExpressionKind::Segno | RepeatExpressionKind::Fine
        );
        if takes_text && mark.is_valid_text(text) {
            mark.text = text.to_string();
            mark.use_symbol = false;
        }
        mark
    }

    /// Which kind of mark this is.
    pub fn kind(&self) -> RepeatExpressionKind {
        self.kind
    }

    /// The words the mark is written in: music21's `getText`.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Writes the mark in other words, whatever they are: music21's
    /// `setText`.
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
    }

    /// Whether these words are one of the ways this kind of mark is
    /// written, ignoring case, spaces and full stops: music21's
    /// `isValidText`.
    pub fn is_valid_text(&self, text: &str) -> bool {
        let strip = |text: &str| -> String {
            text.trim()
                .chars()
                .filter(|character| *character != ' ' && *character != '.')
                .flat_map(char::to_lowercase)
                .collect()
        };
        let text = strip(text);
        self.kind
            .text_alternatives()
            .iter()
            .any(|candidate| strip(candidate) == text)
    }

    /// Whether the mark is drawn as its sign rather than in words.
    pub fn use_symbol(&self) -> bool {
        self.use_symbol
    }

    /// Says whether the mark is drawn as its sign. Only a coda and a segno
    /// have one to draw.
    pub fn set_use_symbol(&mut self, use_symbol: bool) {
        self.use_symbol = use_symbol;
    }

    /// Which side of the staff the words are written on, where a score says.
    pub fn placement(&self) -> Option<Placement> {
        self.placement.0
    }

    /// Says which side of the staff the words are written on.
    pub fn set_placement(&mut self, placement: Option<Placement>) {
        self.placement.0 = placement;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_mark_takes_only_its_own_words() {
        // music21: DaCapo('D.C.') keeps the words, DaCapo('D.S.') does not.
        let da_capo = RepeatExpression::with_text(RepeatExpressionKind::DaCapo, "D.C.");
        assert_eq!(da_capo.text(), "D.C.");
        let wrong = RepeatExpression::with_text(RepeatExpressionKind::DaCapo, "D.S.");
        assert_eq!(wrong.text(), "Da Capo");
        assert!(da_capo.is_valid_text("da capo"));
    }

    #[test]
    fn a_coda_is_a_sign_until_it_is_given_words() {
        assert!(RepeatExpression::new(RepeatExpressionKind::Coda).use_symbol());
        assert!(!RepeatExpression::with_text(RepeatExpressionKind::Coda, "al Coda").use_symbol());
        assert!(RepeatExpression::new(RepeatExpressionKind::Segno).use_symbol());
        assert_eq!(RepeatExpressionKind::Fine.justify(), "right");
        assert_eq!(RepeatExpressionKind::Segno.justify(), "center");
    }
}
