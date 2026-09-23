use std::str::FromStr;

use crate::{
    chord::Chord,
    chord::root::{pitch_class, step_num},
    defaults::{FloatType, IntegerType},
    duration::{Duration, DurationType},
    error::{Error, Result},
    interval::Interval,
    pitch::Pitch,
    stream::{Stream, StreamElement},
};
use std::collections::BTreeSet;

mod figure;
mod parse;
mod realize;
mod tables;

pub use figure::{
    ChordSymbolFigure, chord_symbol_figure_from_chord, chord_symbol_from_chord,
    chord_symbol_kind_from_chord,
};
pub(crate) use figure::{chord_symbol_spellings, chord_symbol_spellings_with_root};
pub use tables::{
    CHORD_KIND_ALIASES, Music21ChordType, abbreviations_for_kind, current_abbreviation_for_kind,
    known_chord_symbol_types, notation_for_kind, resolve_kind_alias,
};

use parse::*;
pub use realize::{
    ChordStepModification, ChordStepModificationType, inversion_is_valid_for_kind,
    sound_chord_kind, sound_chord_notation,
};
use tables::*;

/// Tertian quality parsed from a chord symbol.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ChordQuality {
    /// Major triad or major-family sonority.
    Major,
    /// Minor triad or minor-family sonority.
    Minor,
    /// Dominant seventh-family sonority.
    Dominant,
    /// Diminished triad or diminished-family sonority.
    Diminished,
    /// Augmented triad sonority.
    Augmented,
    /// Half-diminished seventh-family sonority.
    HalfDiminished,
    /// Suspended-second sonority.
    Suspended2,
    /// Suspended-fourth sonority.
    Suspended4,
    /// Power-chord sonority containing a root and fifth.
    Power,
    /// A single pitch: music21's `pedal` kind.
    Pedal,
}

/// A chord-symbol alteration such as `b5` or `#11`.
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ChordAlteration {
    degree: u8,
    semitones: IntegerType,
}

impl ChordAlteration {
    /// Creates an alteration for a scale degree and semitone displacement.
    pub fn new(degree: u8, semitones: IntegerType) -> Self {
        Self { degree, semitones }
    }

    /// Returns the altered or added chord degree.
    pub fn degree(&self) -> u8 {
        self.degree
    }

    /// Returns the semitone displacement from the unaltered degree.
    pub fn semitones(&self) -> IntegerType {
        self.semitones
    }
}

/// Parsed chord symbol.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[must_use]
pub struct ChordSymbol {
    figure: String,
    root: Pitch,
    bass: Option<Pitch>,
    quality: ChordQuality,
    extensions: Vec<u8>,
    alterations: Vec<ChordAlteration>,
    #[cfg_attr(feature = "serde", serde(default))]
    omissions: Vec<u8>,
    #[cfg_attr(feature = "serde", serde(default))]
    additions: Vec<ChordAlteration>,
    /// music21's kind, where the shorthand is one of its abbreviations.
    #[cfg_attr(feature = "serde", serde(default))]
    kind: Option<String>,
    /// What music21 reads after the kind, applied in order when the symbol
    /// is realized.
    #[cfg_attr(feature = "serde", serde(default))]
    modifications: Vec<ChordStepModification>,
    /// How long the symbol holds. A symbol as written takes no time, as
    /// music21's takes none.
    #[cfg_attr(feature = "serde", serde(default = "no_time"))]
    duration: Duration,
}

/// The length of a symbol nobody has given one: none at all.
fn no_time() -> Duration {
    Duration::from_type(DurationType::Zero)
}

/// The kind a shorthand names outright, music21's `_getKindFromShortHand`:
/// the shorthand up to its first `add`, `alter`, `omit` or `subtract`, and
/// up to the first sharp or flat that has a digit after it, matched whole
/// against the abbreviations. With it, how much of the shorthand it took.
fn music21_kind(shorthand: &str) -> Option<(&'static Music21ChordType, usize)> {
    let mut cut = shorthand.len();
    for marker in ["add", "alter", "omit", "subtract"] {
        if let Some(at) = shorthand.find(marker) {
            cut = cut.min(at);
        }
    }
    let mut head = &shorthand[..cut];
    if let Some(at) = head.find('#')
        && head[at + 1..].starts_with(|c: char| c.is_ascii_digit())
    {
        head = &head[..at];
    }
    if let Some(at) = head.find('b')
        && at < head.len() - 1
        && head[at + 1..].starts_with(|c: char| c.is_ascii_digit())
        && !head.contains("ob9")
        && !head.contains("øb9")
    {
        head = &head[..at];
    }
    MUSIC21_CHORD_TYPES
        .iter()
        .find(|chord_type| chord_type.abbreviations.contains(&head))
        .map(|chord_type| (chord_type, head.len()))
}

/// What music21's `_parseFigure` reads out of the shorthand after the kind:
/// the `add`, `alter`, `omit` and `subtract` tokens in the order written,
/// then every sharpened or flattened degree left over, each as an addition.
/// `None` where the leftover is not degrees at all, which music21 refuses.
fn music21_modifications(remaining: &str) -> Option<Vec<ChordStepModification>> {
    const MARKERS: [(&str, ChordStepModificationType); 4] = [
        ("add", ChordStepModificationType::Add),
        ("alter", ChordStepModificationType::Alter),
        ("omit", ChordStepModificationType::Subtract),
        ("subtract", ChordStepModificationType::Subtract),
    ];
    let mut out = Vec::new();
    let first_marker = MARKERS
        .iter()
        .filter_map(|(marker, _)| remaining.find(marker))
        .min();
    if let Some(start) = first_marker {
        let mut text = &remaining[start..];
        while let Some((at, marker, kind)) = MARKERS
            .iter()
            .filter_map(|(marker, kind)| text.find(marker).map(|at| (at, *marker, *kind)))
            .min_by_key(|(at, _, _)| *at)
        {
            let mut rest = &text[at + marker.len()..];
            let mut alter = 0;
            if let Some(after) = rest.strip_prefix('b') {
                alter = -1;
                rest = after;
            } else if let Some(after) = rest.strip_prefix('#') {
                alter = 1;
                rest = after;
            }
            let digits = rest.chars().take_while(char::is_ascii_digit).count().min(2);
            let degree_text = &rest[..digits];
            if let Ok(degree) = degree_text.parse::<u8>() {
                out.push(ChordStepModification::new(kind, degree, alter).ok()?);
            }
            text = &rest[digits..];
        }
    }

    // The degrees before any marker, sharpened or flattened, each an
    // addition: `b9` in `C7b9`, or `#5b9` in `C7#5b9`.
    let before: String = remaining[..first_marker.unwrap_or(remaining.len())].replace(',', "");
    let mut items: Vec<String> = Vec::new();
    if before.contains(['b', '#']) {
        let chars: Vec<char> = before.chars().collect();
        let mut current = String::new();
        let mut index = 0;
        while index < chars.len() {
            if matches!(chars[index], 'b' | '#') {
                if !current.is_empty() {
                    items.push(std::mem::take(&mut current));
                }
                let mut group = String::new();
                while index < chars.len() && matches!(chars[index], 'b' | '#') {
                    group.push(chars[index]);
                    index += 1;
                }
                while index < chars.len() && !matches!(chars[index], 'b' | '#') {
                    group.push(chars[index]);
                    index += 1;
                }
                items.push(group);
            } else {
                current.push(chars[index]);
                index += 1;
            }
        }
        if !current.is_empty() {
            items.push(current);
        }
    } else if !before.is_empty() {
        items.push(before);
    }

    // What is left is degrees and nothing else: music21 reads each as a
    // number once its sharps and flats are off, and refuses anything that
    // is not one.
    if items.iter().any(|item| {
        !item
            .chars()
            .all(|c| c.is_ascii_digit() || matches!(c, 'b' | '#'))
    }) {
        return None;
    }
    let mut tokens: Vec<String> = Vec::new();
    for item in items {
        let digits: String = item.chars().filter(char::is_ascii_digit).collect();
        let number: u32 = digits.parse().ok()?;
        if number > 20 {
            // Several degrees run together, `69` or `b913`: a `1` takes the
            // digit after it, and an accidental waits for the digit after it.
            let chars: Vec<char> = item.chars().collect();
            let mut prefix = String::new();
            let mut skip = false;
            for (index, &ch) in chars.iter().enumerate() {
                if skip {
                    skip = false;
                    continue;
                }
                if ch == '1' {
                    let mut token = String::from(ch);
                    if let Some(next) = chars.get(index + 1) {
                        token.push(*next);
                    }
                    tokens.push(token);
                    skip = true;
                } else if matches!(ch, 'b' | '#') {
                    prefix.push(ch);
                } else {
                    prefix.push(ch);
                    tokens.push(std::mem::take(&mut prefix));
                }
            }
        } else {
            tokens.push(item);
        }
    }
    for token in tokens {
        let alter = if token.contains('b') {
            -(token.matches('b').count() as IntegerType)
        } else {
            token.matches('#').count() as IntegerType
        };
        let digits: String = token
            .chars()
            .skip_while(|c| !c.is_ascii_digit())
            .take_while(char::is_ascii_digit)
            .collect();
        if let Ok(degree) = digits.parse::<u8>() {
            out.push(
                ChordStepModification::new(ChordStepModificationType::Add, degree, alter).ok()?,
            );
        }
    }
    Some(out)
}

impl ChordSymbol {
    /// Parses a chord symbol such as `"Cmaj7"`, `"F#m7b5"`, or `"Bb7#11"`.
    ///
    /// A `b` after the root's letter is read as a flat, as a lead sheet
    /// writes one; [`Self::parse_music21`] reads a figure as music21 does.
    pub fn parse(figure: impl Into<String>) -> Result<Self> {
        Self::parse_with(figure.into(), false)
    }

    /// Parses a figure exactly as music21's `ChordSymbol` reads one, where
    /// a root or a bass is a letter with only `#` and `-` after it. A `b`
    /// is not a flat there: `Bb7` is a B major triad with a flattened
    /// seventh added, and `Ebmaj7` is no figure at all, since `bmaj7` names
    /// no kind.
    ///
    /// The root is the letter at the front, or everything before a comma;
    /// the bass is a `/` and a letter wherever it stands, so `C/B- add 2`
    /// adds a second to a C triad over B flat; and what is left names one
    /// of music21's kinds and its modifications, or is a list of degrees
    /// over no kind at all — `C35b7` is a root with a third, a fifth and a
    /// flat seventh added to it. Anything else is refused with music21's
    /// words.
    pub fn parse_music21(figure: impl Into<String>) -> Result<Self> {
        let figure = figure.into();
        let compact: String = figure.chars().filter(|c| !c.is_whitespace()).collect();
        let refused_root = || {
            Error::Chord(format!(
                "Chord {compact} does not begin with a valid root note."
            ))
        };
        let (root_text, rest) = match compact.find(',') {
            Some(at) => {
                let root = compact[..at].to_string();
                let rest = compact.replace(',', "").replace(root.as_str(), "");
                (root, rest)
            }
            None => {
                let mut letters = compact.char_indices();
                match letters.next() {
                    Some((_, first)) if "ABCDEFGabcdefg".contains(first) => {}
                    _ => return Err(refused_root()),
                }
                let end = letters
                    .find(|(_, c)| !matches!(c, '#' | '-'))
                    .map_or(compact.len(), |(at, _)| at);
                let root = compact[..end].to_string();
                let rest = compact.replacen(root.as_str(), "", 1);
                (root, rest)
            }
        };
        let root = Pitch::from_name(&root_text).map_err(|_| refused_root())?;
        // The bass is `/` and a root-shaped name wherever it stands, and it
        // is taken out of what is left wherever it appears there.
        let mut bass = None;
        let mut remaining = rest.clone();
        if let Some(slash) = compact.find('/') {
            let after = &compact[slash + 1..];
            let mut letters = after.char_indices();
            if let Some((_, first)) = letters.next()
                && "ABCDEFGabcdefg".contains(first)
            {
                let end = letters
                    .find(|(_, c)| !matches!(c, '#' | '-'))
                    .map_or(after.len(), |(at, _)| at);
                let written = &after[..end];
                bass = Some(Pitch::from_name(written)?);
                remaining = rest.replace(&format!("/{written}"), "");
            }
        }
        let (kind, taken) = match music21_kind(&remaining) {
            Some((chord_type, taken)) => (chord_type.kind.to_string(), taken),
            None => (String::new(), 0),
        };
        let refused = || {
            let mut said = remaining[taken..].replace(',', "");
            for marker in ["add", "alter", "omit", "subtract"] {
                if let Some(at) = said.find(marker) {
                    said.truncate(at);
                }
            }
            Error::Chord(format!(
                "Invalid chord abbreviation '{said}'; see music21.harmony.CHORD_TYPES for valid abbreviations or specify all alterations."
            ))
        };
        let modifications = music21_modifications(&remaining[taken..]).ok_or_else(refused)?;
        // The crate's own reading of the letters, which says nothing about
        // the root and so is read over C; music21 carries no such reading
        // and sounds only what the kind and its modifications say.
        let own = Self::parse_with(format!("C{remaining}"), false).ok();
        Ok(Self {
            figure: figure.trim().to_string(),
            root,
            bass,
            quality: own.as_ref().map_or(ChordQuality::Major, |own| own.quality),
            extensions: own
                .as_ref()
                .map(|own| own.extensions.clone())
                .unwrap_or_default(),
            alterations: own.map(|own| own.alterations).unwrap_or_default(),
            omissions: Vec::new(),
            additions: Vec::new(),
            kind: Some(kind),
            modifications,
            duration: no_time(),
        })
    }

    fn parse_with(figure: String, music21: bool) -> Result<Self> {
        let trimmed = figure.trim();
        if trimmed.is_empty() {
            return Err(Error::Chord("chord symbol cannot be empty".to_string()));
        }
        // music21 drops every space before reading a figure, so `C7 omit 3`
        // is `C7omit3`.
        let compact: String = trimmed.chars().filter(|c| !c.is_whitespace()).collect();

        let (body, bass_segment) = match compact.split_once('/') {
            Some((body, bass)) => (body, Some(bass)),
            None => (compact.as_str(), None),
        };
        let body_parts = split_music21_pitch_modifiers(body);
        let bass_parts = bass_segment.map(split_music21_pitch_modifiers);
        let bass = bass_parts
            .as_ref()
            .map(|parts| parse_pitch_only(&parts.base, music21))
            .transpose()?;

        let (root_name, suffix) = parse_pitch_prefix(&body_parts.base, music21)?;
        let root = Pitch::from_name(root_name)?;
        let suffix_without_additions = strip_addition_groups(suffix);
        let mut additions = parse_additions(suffix);
        let mut omissions = parse_omissions(suffix);
        for pitch_name in body_parts
            .additions
            .iter()
            .chain(bass_parts.iter().flat_map(|parts| parts.additions.iter()))
        {
            if let Some(addition) = pitch_name_addition(&root, pitch_name) {
                additions.push(addition);
            }
        }
        for pitch_name in body_parts
            .omissions
            .iter()
            .chain(bass_parts.iter().flat_map(|parts| parts.omissions.iter()))
        {
            if let Some(omission) = pitch_name_degree(&root, pitch_name)
                && !omissions.contains(&omission)
            {
                omissions.push(omission);
            }
        }

        let mut alterations = parse_alterations(&suffix_without_additions);
        add_implicit_music21_alterations(&suffix_without_additions, &mut alterations);
        let extensions = parse_extensions(&suffix_without_additions, &alterations);
        let quality = parse_quality(&suffix_without_additions, &alterations);
        // music21's `_getKindFromShortHand` names the kind outright, and
        // what it leaves is read as music21 reads it; a leftover that is not
        // degrees is a figure music21 refuses, which the crate then reads
        // its own way.
        // The whole of the figure after the root, `add` and `omit` included,
        // which the pitch-modifier split above took off `suffix`.
        let full_suffix = &body[body_parts.base.len() - suffix.len()..];
        let (kind, modifications) = match music21_kind(full_suffix) {
            Some((chord_type, taken)) => match music21_modifications(&full_suffix[taken..]) {
                Some(modifications) => (Some(chord_type.kind.to_string()), modifications),
                None => (None, Vec::new()),
            },
            None => (None, Vec::new()),
        };

        Ok(Self {
            figure: trimmed.to_string(),
            root,
            bass,
            quality,
            extensions,
            alterations,
            omissions,
            additions,
            kind,
            modifications,
            duration: no_time(),
        })
    }

    /// How long the symbol holds. A symbol as written takes no time, which
    /// is music21's default; [`realize_chord_symbol_durations`] gives each
    /// one in a stream the time until the next.
    pub fn duration(&self) -> &Duration {
        &self.duration
    }

    /// Says how long the symbol holds.
    pub fn set_duration(&mut self, duration: Duration) {
        self.duration = duration;
    }

    /// The same symbol holding for a given time.
    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.duration = duration;
        self
    }

    /// music21's `chordKind`: the kind the shorthand names, where it is one
    /// of music21's abbreviations, so `Cmaj7` is `major-seventh` and `CN6`
    /// the Neapolitan. `None` for a shorthand the crate reads on its own.
    pub fn kind(&self) -> Option<&str> {
        self.kind.as_deref()
    }

    /// Returns the original chord-symbol figure.
    pub fn figure(&self) -> &str {
        &self.figure
    }

    /// Returns the root pitch.
    pub fn root(&self) -> &Pitch {
        &self.root
    }

    /// Returns the slash bass pitch, if one was supplied.
    pub fn bass(&self) -> Option<&Pitch> {
        self.bass.as_ref()
    }

    /// Returns the parsed chord quality.
    pub fn quality(&self) -> ChordQuality {
        self.quality
    }

    /// Returns parsed extension degrees.
    pub fn extensions(&self) -> &[u8] {
        &self.extensions
    }

    /// Returns parsed alterations.
    pub fn alterations(&self) -> &[ChordAlteration] {
        &self.alterations
    }

    /// Returns degrees omitted with `no...` or `omit...` markers.
    pub fn omissions(&self) -> &[u8] {
        &self.omissions
    }

    /// Returns parsed added tones from `add(...)` groups.
    pub fn additions(&self) -> &[ChordAlteration] {
        &self.additions
    }

    /// Rebuilds the figure from the parsed parts in one canonical spelling:
    /// music21's `findFigure`. Whatever was typed, a half-diminished seventh
    /// comes back as `m7b5`, an augmented triad as `aug`, an added ninth as
    /// `add(9)`, and the result parses back to the same chord.
    pub fn find_figure(&self) -> String {
        let mut figure = self.root.name();
        figure.push_str(&self.quality_and_extension());
        for alteration in &self.alterations {
            if alteration.degree == 5 && self.fifth_is_implied() {
                continue;
            }
            figure.push_str(&alteration_text(alteration));
        }
        for addition in &self.additions {
            figure.push_str(&format!("add({})", alteration_text(addition)));
        }
        for omission in &self.omissions {
            figure.push_str(&format!("no{omission}"));
        }
        if let Some(bass) = &self.bass
            && bass.name() != self.root.name()
        {
            figure.push('/');
            figure.push_str(&bass.name());
        }
        figure
    }

    /// The same chord on a transposed root and bass, its figure rebuilt by
    /// [`Self::find_figure`]: music21's `transpose`, so `B-/D` up a major
    /// second is `C/E`.
    pub fn transpose(&self, interval: &Interval) -> Result<ChordSymbol> {
        let mut transposed = self.clone();
        transposed.root = self.root.transpose(interval)?;
        transposed.bass = self
            .bass
            .as_ref()
            .map(|bass| bass.transpose(interval))
            .transpose()?;
        transposed.figure = transposed.find_figure();
        Ok(transposed)
    }

    /// Whether the chord can stand in the given inversion: music21's
    /// `inversionIsValid`, which reads the kind — first and second
    /// inversions for anything but a pedal, a third for the sevenths and
    /// the stacked kinds above them, a fourth for the ninths and up, a fifth
    /// for the elevenths and thirteenths. A shorthand naming no kind of
    /// music21's is read by the extensions the crate found in it. Root
    /// position is not an inversion.
    pub fn inversion_is_valid(&self, inversion: u8) -> bool {
        if let Some(kind) = &self.kind {
            return realize::inversion_is_valid_for_kind(kind, inversion);
        }
        let highest = self
            .extensions
            .iter()
            .copied()
            .filter(|degree| matches!(degree, 7 | 9 | 11 | 13))
            .max()
            .unwrap_or(5);
        match inversion {
            1 | 2 => true,
            3 => highest >= 7,
            4 => highest >= 9,
            5 => highest >= 11,
            _ => false,
        }
    }

    /// The degrees added to, taken from or altered in the kind: music21's
    /// `chordStepModifications`, read off the figure or handed over.
    pub fn chord_step_modifications(&self) -> &[ChordStepModification] {
        &self.modifications
    }

    /// Adds a chord-step modification, which the next realization applies:
    /// music21's `addChordStepModification`. One already there is not added
    /// twice.
    pub fn add_chord_step_modification(&mut self, modification: ChordStepModification) {
        if !self.modifications.contains(&modification) {
            self.modifications.push(modification);
        }
    }

    /// Replaces every chord-step modification at once.
    pub fn set_chord_step_modifications(&mut self, modifications: Vec<ChordStepModification>) {
        self.modifications = modifications;
    }

    /// A symbol given as MusicXML gives one: a root, one of music21's chord
    /// kinds, and a bass where it is not the root, rather than a figure.
    /// The figure is written as music21 would write that kind.
    pub fn from_kind(root: Pitch, kind: &str, bass: Option<Pitch>) -> Result<Self> {
        let abbreviation = current_abbreviation_for_kind(kind)
            .ok_or_else(|| Error::Chord(format!("no such chord kind: {kind}")))?;
        let mut figure = format!("{}{abbreviation}", root.name());
        if let Some(bass) = bass.as_ref().filter(|bass| bass.name() != root.name()) {
            figure.push('/');
            figure.push_str(&bass.name());
        }
        let mut symbol = Self::parse(&figure)?;
        symbol.kind = Some(kind.to_string());
        symbol.modifications.clear();
        symbol.root = root;
        if bass.is_some() {
            symbol.bass = bass;
        }
        Ok(symbol)
    }

    /// The same symbol as a chord of another kind, with no modifications:
    /// a kind music21's table has been given while a program runs, which
    /// the crate's own table has not got.
    pub fn with_kind(mut self, kind: &str) -> Self {
        self.kind = Some(kind.to_string());
        self.modifications.clear();
        self
    }

    /// Sets the root the symbol is sounded on.
    pub fn set_root(&mut self, root: Pitch) {
        self.root = root;
    }

    /// Sets the bass, or takes it away with `None`.
    pub fn set_bass(&mut self, bass: Option<Pitch>) {
        self.bass = bass;
    }

    fn fifth_is_implied(&self) -> bool {
        matches!(
            self.quality,
            ChordQuality::Diminished | ChordQuality::Augmented | ChordQuality::HalfDiminished
        )
    }

    /// The quality with the extension it is written with: the highest
    /// extension that no alteration accounts for, so `E7#9` keeps its `7`.
    fn quality_and_extension(&self) -> String {
        let seventh_family = self
            .extensions
            .iter()
            .copied()
            .filter(|degree| matches!(degree, 7 | 9 | 11 | 13))
            .filter(|degree| {
                !self
                    .alterations
                    .iter()
                    .any(|alteration| alteration.degree == *degree)
            })
            .max()
            .or_else(|| {
                self.extensions
                    .iter()
                    .any(|degree| matches!(degree, 7 | 9 | 11 | 13))
                    .then_some(7)
            });
        let sixth = self.extensions.contains(&6);
        let extension = |written: &str| -> String {
            match seventh_family {
                Some(degree) => format!("{written}{degree}"),
                None if sixth => format!("{written}6"),
                None => written.to_string(),
            }
        };
        match self.quality {
            ChordQuality::Major => match seventh_family {
                Some(degree) => format!("maj{degree}"),
                None if sixth => "6".to_string(),
                None => String::new(),
            },
            ChordQuality::Minor => extension("m"),
            ChordQuality::Dominant => seventh_family.unwrap_or(7).to_string(),
            ChordQuality::Diminished => extension("dim"),
            ChordQuality::Augmented => extension("aug"),
            ChordQuality::HalfDiminished => format!("m{}b5", seventh_family.unwrap_or(7)),
            ChordQuality::Suspended2 => match seventh_family {
                Some(degree) => format!("{degree}sus2"),
                None => "sus2".to_string(),
            },
            ChordQuality::Suspended4 => match seventh_family {
                Some(degree) => format!("{degree}sus4"),
                None => "sus4".to_string(),
            },
            ChordQuality::Power => "5".to_string(),
            ChordQuality::Pedal => "pedal".to_string(),
        }
    }

    /// Realizes the symbol as a chord: [`Self::pitches`], sounding
    /// together.
    pub fn to_chord(&self) -> Result<Chord> {
        Chord::new(self.pitches()?.as_slice())
    }

    /// The pitches the symbol sounds, with their octaves, lowest first:
    /// music21's `ChordSymbol._updatePitches`.
    ///
    /// The root is taken from octave three and the kind laid out above it; a
    /// ninth, eleventh or thirteenth has its upper notes lifted an octave; a
    /// bass the kind can invert onto is put under the chord by raising the
    /// notes below it, and one it cannot is added an octave under the root;
    /// the chord-step modifications are applied; and the whole is moved down
    /// until nothing is above the D over middle C, or up until nothing is
    /// below the piano's lowest A. So `C/E` is `E3 G3 C4`, `C11` is `C2 E2 G2
    /// B-2 D3 F3`, and `Gm/F#` is `F#2 G3 B-3 D4`.
    ///
    /// A shorthand naming none of music21's kinds is laid out the same way
    /// from the crate's own reading of it.
    pub fn pitches(&self) -> Result<Vec<Pitch>> {
        if self.kind.as_deref() == Some("") {
            return realize::sound_chord_kind(
                &self.root,
                "",
                self.bass.as_ref(),
                &self.modifications,
            );
        }
        match self.kind_notation() {
            Some(notation) => {
                let mut realized = realize::realize(
                    &self.root,
                    self.bass.as_ref(),
                    self.kind.as_deref(),
                    Some(notation),
                    None,
                    &self.modifications,
                )?;
                self.apply_own_reading(&mut realized.pitches, notation)?;
                Ok(realized.pitches)
            }
            None => {
                let mut intervals = self.spelled_intervals()?;
                intervals.sort_unstable_by_key(|(degree, _)| *degree);
                intervals.dedup();
                let names = intervals
                    .into_iter()
                    .map(|(_, name)| {
                        Ok(Interval::from_name(&name)?
                            .transpose_pitch(&self.root)?
                            .name())
                    })
                    .collect::<Result<Vec<_>>>()?;
                Ok(
                    realize::realize(&self.root, self.bass.as_ref(), None, None, Some(names), &[])?
                        .pitches,
                )
            }
        }
    }

    /// What the crate reads beyond music21 over a kind: degrees omitted with
    /// `no`, and degrees added in `add(...)` groups or by naming a pitch,
    /// which its own figure writer produces.
    fn apply_own_reading(&self, pitches: &mut Vec<Pitch>, notation: &str) -> Result<()> {
        if !self.omissions.is_empty() {
            for (degree, name) in notation_intervals(notation)? {
                if !self.omissions.contains(&degree) {
                    continue;
                }
                let omitted = Interval::from_name(&name)?
                    .transpose_pitch(&self.root)?
                    .name();
                pitches.retain(|pitch| pitch.name() != omitted);
            }
        }
        for addition in &self.additions {
            let (_, name) = added_interval(addition)?;
            let pitch = Interval::from_name(name)?.transpose_pitch(&self.root)?;
            if pitches
                .iter()
                .any(|existing| existing.name() == pitch.name())
            {
                continue;
            }
            // Over the chord, where an added tone is written.
            let mut placed = pitch;
            let top = pitches.last().map_or(0.0, Pitch::ps);
            placed.set_octave(pitches.last().and_then(Pitch::octave));
            while placed.ps() <= top {
                let octave = placed.octave().unwrap_or(3);
                placed.set_octave(Some(octave + 1));
            }
            pitches.push(placed);
        }
        Ok(())
    }

    /// The notation the symbol is realized from when its shorthand names one
    /// of music21's kinds outright: `N6` is the Neapolitan whatever the
    /// crate's own reading of the letters would be.
    fn kind_notation(&self) -> Option<&'static str> {
        notation_for_kind(self.kind.as_deref()?)
    }

    /// The intervals the symbol's quality, extensions, alterations and
    /// additions spell, for a shorthand that names no kind outright.
    fn spelled_intervals(&self) -> Result<Vec<(u8, String)>> {
        let mut intervals = self.base_intervals();

        let highest = self
            .extensions
            .iter()
            .copied()
            .filter(|degree| !self.alterations.iter().any(|alt| alt.degree == *degree))
            .max()
            .unwrap_or(0);
        for extension in [6, 9, 11, 13] {
            let implied = match extension {
                9 => highest >= 11,
                11 => highest == 13,
                _ => false,
            };
            if (self.extensions.contains(&extension) || implied)
                && !self.alterations.iter().any(|alt| alt.degree == extension)
                && !self.omissions.contains(&extension)
            {
                intervals.push((extension, default_extension_interval(extension)));
            }
        }

        for alteration in &self.alterations {
            if alteration.degree == 5 {
                continue;
            }
            let altered = altered_interval(alteration)?;
            // An altered degree stands in place of the one the chord's own
            // quality would give, rather than sounding beside it: the seventh
            // of `F#mM7` is `E#` alone, not `E` and `E#` together.
            intervals.retain(|(degree, _)| *degree != altered.0);
            intervals.push(altered);
        }

        for addition in &self.additions {
            intervals.push(added_interval(addition)?);
        }

        Ok(intervals
            .into_iter()
            .map(|(degree, name)| (degree, name.to_string()))
            .collect())
    }

    fn base_intervals(&self) -> Vec<(u8, &'static str)> {
        let altered_fifth = self
            .alterations
            .iter()
            .find(|alteration| alteration.degree == 5)
            .and_then(|alteration| match alteration.semitones {
                -1 => Some("d5"),
                1 => Some("a5"),
                _ => None,
            });

        let fifth = altered_fifth.unwrap_or("P5");
        let has_seventh = self
            .extensions
            .iter()
            .any(|degree| matches!(degree, 7 | 9 | 11 | 13));

        let intervals = match self.quality {
            ChordQuality::Major => {
                if has_seventh {
                    vec![(1, "P1"), (3, "M3"), (5, fifth), (7, "M7")]
                } else {
                    vec![(1, "P1"), (3, "M3"), (5, fifth)]
                }
            }
            ChordQuality::Minor => {
                if has_seventh {
                    vec![(1, "P1"), (3, "m3"), (5, fifth), (7, "m7")]
                } else {
                    vec![(1, "P1"), (3, "m3"), (5, fifth)]
                }
            }
            ChordQuality::Dominant => vec![(1, "P1"), (3, "M3"), (5, fifth), (7, "m7")],
            ChordQuality::Diminished => {
                if has_seventh {
                    vec![(1, "P1"), (3, "m3"), (5, "d5"), (7, "d7")]
                } else {
                    vec![(1, "P1"), (3, "m3"), (5, "d5")]
                }
            }
            ChordQuality::Augmented => vec![(1, "P1"), (3, "M3"), (5, "a5")],
            ChordQuality::HalfDiminished => vec![(1, "P1"), (3, "m3"), (5, "d5"), (7, "m7")],
            ChordQuality::Suspended2 => vec![(1, "P1"), (2, "M2"), (5, fifth)],
            ChordQuality::Suspended4 => vec![(1, "P1"), (4, "P4"), (5, fifth)],
            ChordQuality::Power => vec![(1, "P1"), (5, fifth)],
            ChordQuality::Pedal => vec![(1, "P1")],
        };

        intervals
            .into_iter()
            .filter(|(degree, _)| !self.omissions.contains(degree))
            .collect()
    }
}

/// Gives every chord symbol in a stream the time it holds for: music21's
/// `realizeChordSymbolDurations`.
///
/// A symbol holds until the next one, wherever in the nesting that is, and
/// the last holds to the end of the stream -- so a single symbol over many
/// notes holds for all of them. A stream with no symbols is left alone.
///
/// music21 hands back the flattened stream, its symbols being the same
/// objects as the score's. A stream here owns what it holds, so the symbols
/// are changed where they sit and [`Stream::flatten`] is there for a caller
/// who wants the flat reading.
///
/// ```
/// use music21_rs::{ChordSymbol, Note, Stream, realize_chord_symbol_durations};
///
/// let mut stream = Stream::new();
/// stream.insert(0.0, ChordSymbol::parse("C")?);
/// stream.insert(2.0, ChordSymbol::parse("G7")?);
/// for beat in 0..8 {
///     stream.insert(f64::from(beat), Note::from_name("C4")?);
/// }
/// realize_chord_symbol_durations(&mut stream);
/// let held: Vec<f64> = stream
///     .events()
///     .iter()
///     .filter(|event| matches!(event.element(), music21_rs::StreamElement::ChordSymbol(_)))
///     .map(|event| event.element().quarter_length())
///     .collect();
/// assert_eq!(held, [2.0, 6.0]);
/// # Ok::<(), music21_rs::Error>(())
/// ```
pub fn realize_chord_symbol_durations(stream: &mut Stream) {
    let lengths = chord_symbol_durations(stream);
    let mut position = 0;
    stream.for_each_mut(&mut |_, element| {
        if let StreamElement::ChordSymbol(symbol) = element
            && let Some((_, length)) = lengths.iter().find(|(at, _)| *at == position)
            && let Ok(duration) = Duration::new(*length)
        {
            symbol.set_duration(duration);
        }
        position += 1;
    });
}

/// How long each chord symbol of a stream is held: its position in
/// [`Stream::leaves`] and the quarter length from it to the next symbol, the
/// last to the end of the stream. This is the reading
/// [`realize_chord_symbol_durations`] writes, for a caller that keeps
/// something of its own beside each element.
///
/// ```
/// use music21_rs::{chordsymbol::chord_symbol_durations, ChordSymbol, Note, Stream};
///
/// let mut stream = Stream::new();
/// stream.insert(0.0, ChordSymbol::parse("C")?);
/// stream.insert(0.0, Note::from_name("C4")?);
/// stream.insert(3.0, ChordSymbol::parse("G7")?);
/// stream.insert(3.0, Note::from_name("B3")?);
/// assert_eq!(chord_symbol_durations(&stream), [(0, 3.0), (2, 1.0)]);
/// # Ok::<(), music21_rs::Error>(())
/// ```
pub fn chord_symbol_durations(stream: &Stream) -> Vec<(usize, FloatType)> {
    let end = stream.end_offset();
    let mut symbols: Vec<(FloatType, usize)> = stream
        .leaves()
        .iter()
        .enumerate()
        .filter(|(_, (_, element))| matches!(element, StreamElement::ChordSymbol(_)))
        .map(|(position, (offset, _))| (*offset, position))
        .collect();
    // Stable, so symbols at one offset keep the order the stream gives them.
    symbols.sort_by(|left, right| left.0.total_cmp(&right.0));
    symbols
        .iter()
        .enumerate()
        .map(|(index, &(offset, position))| {
            let until = symbols.get(index + 1).map_or(end, |next| next.0);
            (position, (until - offset).max(0.0))
        })
        .collect()
}

impl FromStr for ChordSymbol {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self> {
        Self::parse(value)
    }
}

impl TryFrom<&str> for ChordSymbol {
    type Error = Error;

    fn try_from(value: &str) -> Result<Self> {
        Self::parse(value)
    }
}

impl TryFrom<String> for ChordSymbol {
    type Error = Error;

    fn try_from(value: String) -> Result<Self> {
        Self::parse(value)
    }
}

#[cfg(test)]
mod tests {
    /// Every shorthand in music21's table realizes the notes its notation
    /// names, read independently off the root's major scale.
    #[test]
    fn every_music21_kind_realizes_its_own_notation() {
        use super::{ChordSymbol, MUSIC21_CHORD_TYPES};
        use crate::pitch::Accidental;
        use crate::{Key, Pitch};
        use std::collections::BTreeSet;

        let major = Key::from_tonic("C").unwrap();
        for chord_type in MUSIC21_CHORD_TYPES {
            let expected: BTreeSet<String> = chord_type
                .notation
                .split(',')
                .map(|token| {
                    let degree: usize = token.trim_matches(['#', '-']).parse().unwrap();
                    let alter =
                        token.matches('#').count() as f64 - token.matches('-').count() as f64;
                    let mut pitch = major.pitch_from_degree((degree - 1) % 7 + 1).unwrap();
                    if alter != 0.0 {
                        pitch.set_accidental(Some(Accidental::new(alter).unwrap()));
                    }
                    pitch.name()
                })
                .collect();
            for abbreviation in chord_type.abbreviations {
                // music21 reads a sharp or flat with a digit after it as an
                // alteration before it looks the kind up, so `m7b5` is its
                // minor seventh with a lowered fifth; those abbreviations are
                // checked against music21 itself in `chord_symbol_parity`.
                if music21_kind(abbreviation).is_none_or(|(read, _)| read.kind != chord_type.kind) {
                    continue;
                }
                let symbol = ChordSymbol::parse(format!("C{abbreviation}")).unwrap();
                assert_eq!(symbol.kind(), Some(chord_type.kind), "C{abbreviation}");
                let names: BTreeSet<String> = symbol
                    .to_chord()
                    .unwrap()
                    .pitch_names()
                    .into_iter()
                    .collect();
                assert_eq!(names, expected, "C{abbreviation} ({})", chord_type.kind);
            }
        }
        // music21's kind for `C7#11` is the dominant seventh, with the
        // sharpened eleventh added after.
        assert_eq!(
            ChordSymbol::parse("C7#11").unwrap().kind(),
            Some("dominant-seventh")
        );
        assert_eq!(
            ChordSymbol::parse("CN6/E")
                .unwrap()
                .to_chord()
                .unwrap()
                .pitch_names(),
            ["E", "G-", "C", "D-"]
        );
        let _ = Pitch::from_name("C").unwrap();
    }

    /// music21's own `chordSymbolFigureFromChord` examples, the special
    /// kinds and the suspended second in inversion among them.
    #[test]
    fn figures_are_written_as_music21_writes_them() {
        use super::{
            ChordSymbolFigure, chord_symbol_figure_from_chord, chord_symbol_kind_from_chord,
        };
        use crate::{Chord, Pitch};

        let figure =
            |notes: &str| chord_symbol_figure_from_chord(&Chord::new(notes).unwrap()).unwrap();
        let kind = |notes: &str| chord_symbol_kind_from_chord(&Chord::new(notes).unwrap());
        assert_eq!(figure("F3 A3 C#4 E-4 G4 B-4").as_deref(), Some("F+11"));
        assert_eq!(kind("F3 A3 C#4 E-4 G4 B-4"), Some("augmented-11th"));
        assert_eq!(figure("C3 E3 B3 D4").as_deref(), Some("CM9"));
        assert_eq!(figure("C3 D-3 E3 G-3").as_deref(), Some("CN6"));
        assert_eq!(kind("C3 D-3 E3 G-3"), Some("Neapolitan"));
        assert_eq!(figure("C3 D3 G3").as_deref(), Some("Csus2"));
        assert_eq!(figure("C3 E3 G3 D-4").as_deref(), Some("CaddD-"));
        assert_eq!(figure("C3").as_deref(), Some("Cpedal"));
        assert_eq!(figure("").as_deref(), Some(""));

        // A suspended second in inversion is a suspended fourth on the bass.
        let inverted = Chord::new("C3 F3 G3").unwrap();
        let parts = ChordSymbolFigure::from_chord(&inverted).unwrap();
        assert_eq!(parts.root, "C");
        assert_eq!(
            (parts.kind, parts.abbreviation, parts.bass.as_deref()),
            ("suspended-fourth", "sus", None)
        );
        assert_eq!(parts.to_string(), "Csus");
        assert_eq!(parts.written_with("sus4"), "Csus4");

        // The augmented sixths are named from a root the caller fixes.
        let with_root = |notes: &str, root: &str| {
            ChordSymbolFigure::from_chord_with_root(
                &Chord::new(notes).unwrap(),
                &Pitch::from_name(root).unwrap(),
            )
            .unwrap()
        };
        assert_eq!(with_root("C3 F#3 A-3", "C3").to_string(), "CIt+6");
        assert_eq!(with_root("C3 D3 F#3 A-3", "C3").to_string(), "CFr+6");
        assert_eq!(with_root("C3 E-3 F#3 A-3", "C3").to_string(), "CGr+6");
        assert_eq!(with_root("F2 B2 D#3 G#3", "F2").to_string(), "Ftristan");
        assert_eq!(with_root("F2 B2 D#3 G#3", "F2").kind, "Tristan");

        // An inversion writes the bass, and additions are written with the
        // notes the kind lacks beside them.
        let parts = ChordSymbolFigure::from_chord(&Chord::new("E3 G3 C4 B-4").unwrap()).unwrap();
        assert_eq!(parts.to_string(), "C7/E");
        // A bass the kind does not carry is written as the bass alone.
        assert_eq!(figure("D3 C4 E-4 G4").as_deref(), Some("Cm/D"));
        assert!(ChordSymbolFigure::from_chord(&Chord::new("C4 C~4").unwrap()).is_none());
        assert!(ChordSymbolFigure::from_chord(&Chord::new("").unwrap()).is_none());
    }

    #[test]
    fn a_symbol_is_parsed_through_try_from_and_reports_its_alterations() {
        use super::{ChordSymbol, chord_symbol_kind_from_chord};
        use crate::chord::Chord;

        let symbol = ChordSymbol::try_from("C7#11").unwrap();
        assert_eq!(symbol.alterations().len(), 1);
        assert_eq!(symbol.alterations()[0].degree(), 11);
        assert_eq!(symbol.alterations()[0].semitones(), 1);
        assert!(symbol.omissions().is_empty());
        let omitting = ChordSymbol::try_from("C[no3]".to_string()).unwrap();
        assert_eq!(omitting.omissions(), [3]);
        assert!(ChordSymbol::try_from("").is_err());

        assert_eq!(
            chord_symbol_kind_from_chord(&Chord::new("C").unwrap()),
            Some("pedal")
        );
        assert_eq!(
            chord_symbol_kind_from_chord(&Chord::new("C G").unwrap()),
            Some("power")
        );
        assert_eq!(
            chord_symbol_kind_from_chord(&Chord::new("C E G").unwrap()),
            Some("major")
        );
        assert_eq!(chord_symbol_kind_from_chord(&Chord::new("").unwrap()), None);
    }

    #[test]
    fn chord_symbol_figures_match_music21() {
        let cases: [(&str, Option<&str>); 26] = [
            ("C4 E4 G4", Some("C")),
            ("C4 E-4 G4", Some("Cm")),
            ("C4 E4 G#4", Some("C+")),
            ("C4 E-4 G-4", Some("Cdim")),
            ("C4 E4 G4 B-4", Some("C7")),
            ("C4 E4 G4 B4", Some("Cmaj7")),
            ("C4 E-4 G4 B-4", Some("Cm7")),
            ("C4 E-4 G-4 B--4", Some("Co7")),
            ("C4 E-4 G-4 B-4", Some("C\u{00f8}7")),
            ("E4 G4 C5", Some("C/E")),
            ("G3 C4 E4", Some("C/G")),
            ("E4 G4 B-4 C5", Some("C7/E")),
            ("C4 E4 G4 B-4 D5", Some("C9")),
            ("C4 E4 G4 B4 D5", Some("CM9")),
            ("C4 D4 G4", Some("Csus2")),
            ("C4 F4 G4", Some("Csus")),
            ("C4", Some("Cpedal")),
            ("C4 G4", Some("Cpower")),
            ("C4 E4 G4 B-4 D5 F5", Some("C11")),
            ("C4 E4 G4 B-4 D5 F5 A5", Some("C13")),
            ("C4 E4 G4 A4", Some("Am7/C")),
            ("C4 E-4 G4 A4", Some("A\u{00f8}7/C")),
            ("C4 E4 G4 D5", Some("CaddD")),
            ("F4 A4 C5 D5", Some("Dm7/F")),
            ("B3 D4 F4", Some("Bdim")),
            ("C4 D-4 E4", None),
        ];
        for (notes, expected) in cases {
            let chord = Chord::new(notes).unwrap();
            assert_eq!(
                chord_symbol_figure_from_chord(&chord).unwrap().as_deref(),
                expected,
                "{notes}"
            );
        }
        assert_eq!(
            chord_symbol_figure_from_chord(&Chord::empty())
                .unwrap()
                .as_deref(),
            Some("")
        );

        let symbol = chord_symbol_from_chord(&Chord::new("E4 G4 B-4 C5").unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(symbol.figure(), "C7/E");
        assert_eq!(symbol.root().name(), "C");
        assert_eq!(symbol.bass().map(Pitch::name).as_deref(), Some("E"));
        assert!(
            chord_symbol_from_chord(&Chord::new("C4 D-4 E4").unwrap())
                .unwrap()
                .is_none()
        );
        assert!(chord_symbol_from_chord(&Chord::empty()).unwrap().is_none());
    }

    #[test]
    fn kind_lookups_match_music21() {
        assert_eq!(
            abbreviations_for_kind("dominant-seventh"),
            Some(&["7", "dom7"][..])
        );
        assert_eq!(notation_for_kind("dominant-seventh"), Some("1,3,5,-7"));
        assert_eq!(current_abbreviation_for_kind("dominant-seventh"), Some("7"));
        assert_eq!(abbreviations_for_kind("major"), Some(&["", "M", "maj"][..]));
        assert_eq!(current_abbreviation_for_kind("major"), Some(""));
        assert_eq!(abbreviations_for_kind("nonsense"), None);
        for chord_type in known_chord_symbol_types() {
            assert_eq!(
                chord_type.abbreviations.first(),
                Some(&chord_type.abbreviation)
            );
        }
    }

    #[test]
    fn find_figure_writes_one_canonical_spelling_that_parses_back() {
        let cases = [
            ("C", "C", "D"),
            ("Cm7", "Cm7", "Dm7"),
            ("F#dim", "F#dim", "G#dim"),
            ("B-/D", "B-/D", "C/E"),
            ("G7/B", "G7/B", "A7/C#"),
            ("Am/C", "Am/C", "Bm/D"),
            ("Cmaj7", "Cmaj7", "Dmaj7"),
            ("Dm7b5", "Dm7b5", "Em7b5"),
            ("C\u{00f8}7", "Cm7b5", "Dm7b5"),
            ("E7#9", "E7#9", "F#7#9"),
            ("Csus4", "Csus4", "Dsus4"),
            ("G7sus4", "G7sus4", "A7sus4"),
            ("Cadd(9)", "Cadd(9)", "Dadd(9)"),
            ("C6", "C6", "D6"),
            ("Cm6", "Cm6", "Dm6"),
            ("Cdim7", "Cdim7", "Ddim7"),
            ("Caug", "Caug", "Daug"),
            ("C+", "Caug", "Daug"),
            ("C9", "C9", "D9"),
            ("Cmaj7#11", "Cmaj7#11", "Dmaj7#11"),
            ("C5", "C5", "D5"),
            ("Cno3", "Cno3", "Dno3"),
        ];
        let whole_tone = Interval::from_name("M2").unwrap();
        for (typed, canonical, transposed) in cases {
            let symbol = ChordSymbol::parse(typed).unwrap();
            assert_eq!(symbol.find_figure(), canonical, "{typed}");
            let reparsed = ChordSymbol::parse(symbol.find_figure()).unwrap();
            assert_eq!(reparsed.quality(), symbol.quality(), "{typed}");
            assert_eq!(
                reparsed.to_chord().unwrap().pitch_names(),
                symbol.to_chord().unwrap().pitch_names(),
                "{typed}"
            );
            let up = symbol.transpose(&whole_tone).unwrap();
            assert_eq!(up.figure(), transposed, "{typed}");
            assert_eq!(up.find_figure(), transposed, "{typed}");
        }
    }

    #[test]
    fn inversion_validity_follows_the_chord_size() {
        let valid = |figure: &str| -> Vec<u8> {
            let symbol = ChordSymbol::parse(figure).unwrap();
            (0..=6).filter(|n| symbol.inversion_is_valid(*n)).collect()
        };
        assert_eq!(valid("C"), [1, 2]);
        assert_eq!(valid("C6"), [1, 2]);
        assert_eq!(valid("Cm7"), [1, 2, 3]);
        assert_eq!(valid("C9"), [1, 2, 3, 4]);
        assert_eq!(valid("C13"), [1, 2, 3, 4, 5]);
        // music21 reads the kind, and `E7#9` is a dominant seventh with a
        // raised ninth added: it inverts as a seventh does.
        assert_eq!(valid("E7#9"), [1, 2, 3]);
        // A figure music21 refuses is read by the extensions the crate
        // found in it.
        assert_eq!(valid("Cmaj11"), [1, 2, 3, 4, 5]);
    }
    use super::*;

    #[test]
    fn parses_major_seventh_symbol() {
        let symbol: ChordSymbol = "Cmaj7".parse().unwrap();
        assert_eq!(symbol.root().name(), "C");
        assert_eq!(symbol.quality(), ChordQuality::Major);
        assert_eq!(symbol.extensions(), &[7]);
        assert_eq!(
            symbol.to_chord().unwrap().pitched_common_name(),
            "C-major seventh chord"
        );
    }

    #[test]
    fn parses_half_diminished_symbol() {
        let symbol = ChordSymbol::parse("F#m7b5").unwrap();
        assert_eq!(symbol.root().name(), "F#");
        assert_eq!(symbol.quality(), ChordQuality::HalfDiminished);
        assert_eq!(
            symbol.to_chord().unwrap().pitched_common_name(),
            "F#-half-diminished seventh chord"
        );
    }

    #[test]
    fn parses_dominant_altered_symbol() {
        let symbol = ChordSymbol::parse("Bb7#11").unwrap();
        assert_eq!(symbol.root().name(), "B-");
        assert_eq!(symbol.quality(), ChordQuality::Dominant);
        assert_eq!(symbol.extensions(), &[7, 11]);
        assert_eq!(symbol.alterations()[0], ChordAlteration::new(11, 1));
        let names = symbol
            .to_chord()
            .unwrap()
            .pitches()
            .iter()
            .map(Pitch::name)
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["B-", "D", "F", "A-", "E"]);
    }

    #[test]
    fn parses_added_tones_without_changing_the_base_chord() {
        let symbol = ChordSymbol::parse("Cdim9 add(#5)").unwrap();
        assert_eq!(symbol.extensions(), &[9]);
        assert_eq!(symbol.additions(), &[ChordAlteration::new(5, 1)]);
        assert_eq!(
            symbol.to_chord().unwrap().pitch_classes(),
            vec![0, 2, 3, 6, 8, 9]
        );
    }

    #[test]
    fn parses_altered_dominant_with_slash_bass() {
        let symbol = ChordSymbol::parse("D7b9#11/C").unwrap();
        assert_eq!(symbol.root().name(), "D");
        assert_eq!(symbol.bass().map(Pitch::name).as_deref(), Some("C"));
        assert_eq!(symbol.quality(), ChordQuality::Dominant);
        assert_eq!(symbol.extensions(), &[7, 9, 11]);
        assert_eq!(
            symbol.alterations(),
            &[ChordAlteration::new(9, -1), ChordAlteration::new(11, 1)]
        );
        assert_eq!(
            symbol.to_chord().unwrap().pitch_classes(),
            vec![0, 2, 3, 6, 8, 9]
        );
    }

    #[test]
    fn parses_music21_pitch_name_additions() {
        let symbol = ChordSymbol::parse("Ddom7dim5/CaddA,E-").unwrap();

        assert_eq!(symbol.root().name(), "D");
        assert_eq!(symbol.bass().map(Pitch::name).as_deref(), Some("C"));
        assert_eq!(symbol.quality(), ChordQuality::Dominant);
        assert_eq!(symbol.alterations(), &[ChordAlteration::new(5, -1)]);
        assert_eq!(
            symbol.additions(),
            &[ChordAlteration::new(5, 0), ChordAlteration::new(9, -1)]
        );
        assert_eq!(
            symbol.to_chord().unwrap().pitch_classes(),
            vec![0, 2, 3, 6, 8, 9]
        );
    }

    #[test]
    fn generates_petrushka_chord_symbol_name() {
        let chord = Chord::new("C4 D4 Eb4 F#4 Ab4 A4").unwrap();
        let names = chord_symbol_spellings(&chord);

        assert_eq!(
            names.first().map(String::as_str),
            Some("Ddom7dim5/CaddA,E-")
        );
        assert!(names.iter().any(|name| name == "Ddom7dim5/CaddA,E-"));
    }

    #[test]
    fn generates_common_chord_symbols() {
        let major_seventh = Chord::new("C E G B").unwrap();
        let dominant_ninth = Chord::new("C E G B- D").unwrap();

        assert_eq!(
            chord_symbol_spellings(&major_seventh)
                .first()
                .map(String::as_str),
            Some("Cmaj7")
        );
        assert_eq!(
            chord_symbol_spellings(&dominant_ninth)
                .first()
                .map(String::as_str),
            Some("C9")
        );
    }

    #[test]
    fn split_third_triads_do_not_spell_lower_third_as_sharp_nine() {
        let split_third = Chord::new("D4 A4 F#4 F4").unwrap();
        let names = chord_symbol_spellings(&split_third);

        assert_eq!(names.first().map(String::as_str), Some("DaddF"));
        assert!(!names.iter().any(|name| name == "D add(#9)"));
    }

    #[test]
    fn altered_dominants_use_music21_pitch_name_additions() {
        let altered_dominant = Chord::new("C4 E4 G4 Bb4 Eb5").unwrap();
        let names = chord_symbol_spellings(&altered_dominant);

        assert_eq!(names.first().map(String::as_str), Some("C7addE-"));
    }

    #[test]
    fn unrecognized_music21_figures_return_no_symbol() {
        let chord = Chord::new("F4 C5 D5 E-5").unwrap();
        let names = chord_symbol_spellings(&chord);

        assert!(names.is_empty());
    }

    #[test]
    fn generates_music21_figures_with_explicit_root() {
        let major_triad = Chord::new("G3 C4 E4").unwrap();
        let dominant_seventh = Chord::new("G3 B-3 C4 E4").unwrap();
        let power_chord = Chord::new("C4 G4").unwrap();
        let unsupported_dyad = Chord::new("C4 A4").unwrap();

        assert_eq!(
            chord_symbol_spellings_with_root(&major_triad, 0)
                .first()
                .map(String::as_str),
            Some("C/G")
        );
        assert_eq!(
            chord_symbol_spellings_with_root(&dominant_seventh, 0)
                .first()
                .map(String::as_str),
            Some("C7/G")
        );
        assert_eq!(
            chord_symbol_spellings_with_root(&power_chord, 0)
                .first()
                .map(String::as_str),
            Some("Cpower")
        );
        assert!(chord_symbol_spellings_with_root(&unsupported_dyad, 0).is_empty());
    }

    #[test]
    fn dense_sets_follow_music21_fallback_matching() {
        let chord = Chord::new("C4 D-4 E-4 E4 F#4 G4 A-4 A4").unwrap();

        assert_eq!(
            chord_symbol_spellings(&chord).first().map(String::as_str),
            Some("CsusaddA,A-,D-,E,E-,F#,omitF")
        );
    }

    /// The lengths are music21's, read off `realizeChordSymbolDurations`
    /// over the same three bars.
    #[test]
    fn chord_symbols_hold_until_the_next_one_across_bars() {
        use crate::{Note, Stream, StreamElement, StreamKind};

        let bar = |figure: &str, at: FloatType| {
            let mut measure = Stream::with_kind(StreamKind::Measure);
            measure.insert(at, ChordSymbol::parse(figure).unwrap());
            // Appended, so the four notes start where the symbol stands, as
            // music21's `repeatAppend` starts them.
            for _ in 0..4 {
                measure.push(Note::from_name("C4").unwrap());
            }
            measure
        };
        let held = |stream: &Stream| -> Vec<FloatType> {
            stream
                .flatten()
                .events()
                .iter()
                .filter(|event| matches!(event.element(), StreamElement::ChordSymbol(_)))
                .map(|event| event.element().quarter_length())
                .collect()
        };

        let mut part = Stream::with_kind(StreamKind::Part);
        part.insert(0.0, bar("C", 0.0));
        part.insert(4.0, bar("G7", 1.5));
        part.insert(8.0, bar("Am", 2.0));
        assert_eq!(held(&part), [0.0, 0.0, 0.0]);
        realize_chord_symbol_durations(&mut part);
        assert_eq!(held(&part), [5.5, 4.5, 4.0]);

        // One symbol holds to the end of everything after it.
        let mut stream = Stream::new();
        stream.insert(1.0, ChordSymbol::parse("C").unwrap());
        for beat in 1..7 {
            stream.insert(FloatType::from(beat), Note::from_name("C4").unwrap());
        }
        realize_chord_symbol_durations(&mut stream);
        assert_eq!(held(&stream), [6.0]);

        // And a stream with none is left as it was.
        let mut bare = Stream::new();
        bare.push(Note::from_name("C4").unwrap());
        realize_chord_symbol_durations(&mut bare);
        assert_eq!(bare.len(), 1);
    }
}
