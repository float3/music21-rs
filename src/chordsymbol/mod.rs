use std::str::FromStr;

use crate::{
    chord::Chord,
    chord::root::{pitch_class, step_num},
    defaults::{FloatType, IntegerType},
    error::{Error, Result},
    interval::Interval,
    pitch::Pitch,
};
use std::collections::BTreeSet;

mod figure;
mod parse;
mod tables;

pub use figure::{
    ChordSymbolFigure, chord_symbol_figure_from_chord, chord_symbol_from_chord,
    chord_symbol_kind_from_chord,
};
pub(crate) use figure::{chord_symbol_spellings, chord_symbol_spellings_with_root};
pub use tables::{
    Music21ChordType, abbreviations_for_kind, current_abbreviation_for_kind,
    known_chord_symbol_types, notation_for_kind,
};

use parse::*;
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
}

impl ChordSymbol {
    /// Parses a chord symbol such as `"Cmaj7"`, `"F#m7b5"`, or `"Bb7#11"`.
    pub fn parse(figure: impl Into<String>) -> Result<Self> {
        let figure = figure.into();
        let trimmed = figure.trim();
        if trimmed.is_empty() {
            return Err(Error::Chord("chord symbol cannot be empty".to_string()));
        }

        let (body, bass_segment) = match trimmed.split_once('/') {
            Some((body, bass)) => (body, Some(bass)),
            None => (trimmed, None),
        };
        let body_parts = split_music21_pitch_modifiers(body);
        let bass_parts = bass_segment.map(split_music21_pitch_modifiers);
        let bass = bass_parts
            .as_ref()
            .map(|parts| parse_pitch_only(&parts.base))
            .transpose()?;

        let (root_name, suffix) = parse_pitch_prefix(&body_parts.base)?;
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
        // music21's `_getKindFromShortHand`: a shorthand that is one of its
        // abbreviations names the kind outright.
        let shorthand: &str = &suffix_without_additions;
        let kind = MUSIC21_CHORD_TYPES
            .iter()
            .find(|chord_type| chord_type.abbreviations.contains(&shorthand))
            .map(|chord_type| chord_type.kind.to_string());

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
        })
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

    /// Whether the chord has enough members for the given inversion:
    /// music21's `inversionIsValid`, so first and second inversions always
    /// are, a third needs a seventh, a fourth a ninth and a fifth an
    /// eleventh or thirteenth. Root position is not an inversion.
    pub fn inversion_is_valid(&self, inversion: u8) -> bool {
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

    /// Realizes the symbol as a chord. An eleventh implies the ninth and a
    /// thirteenth implies the ninth and eleventh, as music21's chord kinds
    /// spell them, unless the figure omits them.
    pub fn to_chord(&self) -> Result<Chord> {
        let mut intervals: Vec<(u8, String)> = match self.kind_notation() {
            Some(notation) => {
                let mut intervals = notation_intervals(notation)?;
                intervals.retain(|(degree, _)| !self.omissions.contains(degree));
                for addition in &self.additions {
                    let (degree, name) = added_interval(addition)?;
                    intervals.push((degree, name.to_string()));
                }
                intervals
            }
            None => self.spelled_intervals()?,
        };
        intervals.sort_unstable_by_key(|(degree, _)| *degree);
        intervals.dedup();

        let mut pitches = intervals
            .into_iter()
            .map(|(_, name)| Interval::from_name(&name)?.transpose_pitch(&self.root))
            .collect::<Result<Vec<_>>>()?;

        if let Some(bass) = &self.bass {
            if let Some(index) = pitches.iter().position(|pitch| pitch.name() == bass.name()) {
                let bass = pitches.remove(index);
                pitches.insert(0, bass);
            } else {
                pitches.insert(0, bass.clone());
            }
        }

        Chord::new(pitches.as_slice())
    }

    /// The notation the symbol is realized from when its shorthand names one
    /// of music21's kinds outright and nothing alters it: `N6` is the
    /// Neapolitan whatever the crate's own reading of the letters would be.
    fn kind_notation(&self) -> Option<&'static str> {
        if !self.alterations.is_empty() {
            return None;
        }
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
        assert_eq!(ChordSymbol::parse("C7#11").unwrap().kind(), None);
        assert_eq!(
            ChordSymbol::parse("CN6/E")
                .unwrap()
                .to_chord()
                .unwrap()
                .pitch_names(),
            ["E", "C", "D-", "G-"]
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
        assert_eq!(valid("Cmaj11"), [1, 2, 3, 4, 5]);
        assert_eq!(valid("C13"), [1, 2, 3, 4, 5]);
        assert_eq!(valid("E7#9"), [1, 2, 3, 4]);
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
            Some("CsusaddA,A-,D-,E,E-,F#omitF")
        );
    }
}
