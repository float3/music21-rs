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

/// A chord type from music21's `harmony.CHORD_TYPES` table.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Music21ChordType {
    /// music21's kind name, such as `"dominant-seventh"`.
    pub kind: &'static str,
    /// Scale-degree notation, such as `"1,3,5,-7"`.
    pub notation: &'static str,
    /// music21's first abbreviation for this kind, such as `"7"`, which is
    /// the one it uses when writing a figure.
    pub abbreviation: &'static str,
    /// Every abbreviation music21 accepts for this kind, first one first.
    pub abbreviations: &'static [&'static str],
}

/// Returns every chord type this crate knows from music21's harmony tables.
///
/// The table mirrors music21's `harmony.CHORD_TYPES` and is verified against it
/// by `python-parity`'s `chord_type_parity` test.
pub fn known_chord_symbol_types() -> &'static [Music21ChordType] {
    MUSIC21_CHORD_TYPES
}

fn chord_type_named(kind: &str) -> Option<&'static Music21ChordType> {
    MUSIC21_CHORD_TYPES
        .iter()
        .find(|chord_type| chord_type.kind == kind)
}

/// Every abbreviation music21 accepts for a chord kind: music21's
/// `getAbbreviationListGivenChordType`, so `dominant-seventh` gives `7` and
/// `dom7`. `None` for an unknown kind.
pub fn abbreviations_for_kind(kind: &str) -> Option<&'static [&'static str]> {
    chord_type_named(kind).map(|chord_type| chord_type.abbreviations)
}

/// The scale-degree notation of a chord kind: music21's
/// `getNotationStringGivenChordType`, `1,3,5,-7` for `dominant-seventh`.
pub fn notation_for_kind(kind: &str) -> Option<&'static str> {
    chord_type_named(kind).map(|chord_type| chord_type.notation)
}

/// The abbreviation music21 writes a chord kind with: its
/// `getCurrentAbbreviationFor`, the first of [`abbreviations_for_kind`].
pub fn current_abbreviation_for_kind(kind: &str) -> Option<&'static str> {
    chord_type_named(kind).map(|chord_type| chord_type.abbreviation)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Music21Degree {
    degree: u8,
    semitone: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Music21FigureMatch {
    kind: &'static str,
    notation: &'static str,
    abbreviation: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Music21ChordAnalysis {
    d3: Option<u8>,
    d5: Option<u8>,
    d7: Option<u8>,
    d9: Option<u8>,
    d11: Option<u8>,
    d13: Option<u8>,
    is_triad: bool,
    is_seventh: bool,
}

const MUSIC21_CHORD_TYPES: &[Music21ChordType] = &[
    Music21ChordType {
        kind: "major",
        notation: "1,3,5",
        abbreviation: "",
        abbreviations: &["", "M", "maj"],
    },
    Music21ChordType {
        kind: "minor",
        notation: "1,-3,5",
        abbreviation: "m",
        abbreviations: &["m", "min"],
    },
    Music21ChordType {
        kind: "augmented",
        notation: "1,3,#5",
        abbreviation: "+",
        abbreviations: &["+", "aug"],
    },
    Music21ChordType {
        kind: "diminished",
        notation: "1,-3,-5",
        abbreviation: "dim",
        abbreviations: &["dim", "o"],
    },
    Music21ChordType {
        kind: "dominant-seventh",
        notation: "1,3,5,-7",
        abbreviation: "7",
        abbreviations: &["7", "dom7"],
    },
    Music21ChordType {
        kind: "major-seventh",
        notation: "1,3,5,7",
        abbreviation: "maj7",
        abbreviations: &["maj7", "M7"],
    },
    Music21ChordType {
        kind: "minor-major-seventh",
        notation: "1,-3,5,7",
        abbreviation: "mM7",
        abbreviations: &["mM7", "m#7", "minmaj7"],
    },
    Music21ChordType {
        kind: "minor-seventh",
        notation: "1,-3,5,-7",
        abbreviation: "m7",
        abbreviations: &["m7", "min7"],
    },
    Music21ChordType {
        kind: "augmented-major-seventh",
        notation: "1,3,#5,7",
        abbreviation: "+M7",
        abbreviations: &["+M7", "augmaj7"],
    },
    Music21ChordType {
        kind: "augmented-seventh",
        notation: "1,3,#5,-7",
        abbreviation: "7+",
        abbreviations: &["7+", "+7", "aug7"],
    },
    Music21ChordType {
        kind: "half-diminished-seventh",
        notation: "1,-3,-5,-7",
        abbreviation: "ø7",
        abbreviations: &["ø7", "m7b5"],
    },
    Music21ChordType {
        kind: "diminished-seventh",
        notation: "1,-3,-5,--7",
        abbreviation: "o7",
        abbreviations: &["o7", "dim7"],
    },
    Music21ChordType {
        kind: "seventh-flat-five",
        notation: "1,3,-5,-7",
        abbreviation: "dom7dim5",
        abbreviations: &["dom7dim5"],
    },
    Music21ChordType {
        kind: "major-sixth",
        notation: "1,3,5,6",
        abbreviation: "6",
        abbreviations: &["6"],
    },
    Music21ChordType {
        kind: "minor-sixth",
        notation: "1,-3,5,6",
        abbreviation: "m6",
        abbreviations: &["m6", "min6"],
    },
    Music21ChordType {
        kind: "major-ninth",
        notation: "1,3,5,7,9",
        abbreviation: "M9",
        abbreviations: &["M9", "Maj9"],
    },
    Music21ChordType {
        kind: "dominant-ninth",
        notation: "1,3,5,-7,9",
        abbreviation: "9",
        abbreviations: &["9", "dom9"],
    },
    Music21ChordType {
        kind: "minor-major-ninth",
        notation: "1,-3,5,7,9",
        abbreviation: "mM9",
        abbreviations: &["mM9", "minmaj9"],
    },
    Music21ChordType {
        kind: "minor-ninth",
        notation: "1,-3,5,-7,9",
        abbreviation: "m9",
        abbreviations: &["m9", "min9"],
    },
    Music21ChordType {
        kind: "augmented-major-ninth",
        notation: "1,3,#5,7,9",
        abbreviation: "+M9",
        abbreviations: &["+M9", "augmaj9"],
    },
    Music21ChordType {
        kind: "augmented-dominant-ninth",
        notation: "1,3,#5,-7,9",
        abbreviation: "9#5",
        abbreviations: &["9#5", "+9", "aug9"],
    },
    Music21ChordType {
        kind: "half-diminished-ninth",
        notation: "1,-3,-5,-7,9",
        abbreviation: "ø9",
        abbreviations: &["ø9"],
    },
    Music21ChordType {
        kind: "half-diminished-minor-ninth",
        notation: "1,-3,-5,-7,-9",
        abbreviation: "øb9",
        abbreviations: &["øb9"],
    },
    Music21ChordType {
        kind: "diminished-ninth",
        notation: "1,-3,-5,--7,9",
        abbreviation: "o9",
        abbreviations: &["o9", "dim9"],
    },
    Music21ChordType {
        kind: "diminished-minor-ninth",
        notation: "1,-3,-5,--7,-9",
        abbreviation: "ob9",
        abbreviations: &["ob9", "dimb9"],
    },
    Music21ChordType {
        kind: "dominant-11th",
        notation: "1,3,5,-7,9,11",
        abbreviation: "11",
        abbreviations: &["11", "dom11"],
    },
    Music21ChordType {
        kind: "major-11th",
        notation: "1,3,5,7,9,11",
        abbreviation: "M11",
        abbreviations: &["M11", "Maj11"],
    },
    Music21ChordType {
        kind: "minor-major-11th",
        notation: "1,-3,5,7,9,11",
        abbreviation: "mM11",
        abbreviations: &["mM11", "minmaj11"],
    },
    Music21ChordType {
        kind: "minor-11th",
        notation: "1,-3,5,-7,9,11",
        abbreviation: "m11",
        abbreviations: &["m11", "min11"],
    },
    Music21ChordType {
        kind: "augmented-major-11th",
        notation: "1,3,#5,7,9,11",
        abbreviation: "+M11",
        abbreviations: &["+M11", "augmaj11"],
    },
    Music21ChordType {
        kind: "augmented-11th",
        notation: "1,3,#5,-7,9,11",
        abbreviation: "+11",
        abbreviations: &["+11", "aug11"],
    },
    Music21ChordType {
        kind: "half-diminished-11th",
        notation: "1,-3,-5,-7,9,11",
        abbreviation: "ø11",
        abbreviations: &["ø11"],
    },
    Music21ChordType {
        kind: "diminished-11th",
        notation: "1,-3,-5,--7,9,11",
        abbreviation: "o11",
        abbreviations: &["o11", "dim11"],
    },
    Music21ChordType {
        kind: "major-13th",
        notation: "1,3,5,7,9,11,13",
        abbreviation: "M13",
        abbreviations: &["M13", "Maj13"],
    },
    Music21ChordType {
        kind: "dominant-13th",
        notation: "1,3,5,-7,9,11,13",
        abbreviation: "13",
        abbreviations: &["13", "dom13"],
    },
    Music21ChordType {
        kind: "minor-major-13th",
        notation: "1,-3,5,7,9,11,13",
        abbreviation: "mM13",
        abbreviations: &["mM13", "minmaj13"],
    },
    Music21ChordType {
        kind: "minor-13th",
        notation: "1,-3,5,-7,9,11,13",
        abbreviation: "m13",
        abbreviations: &["m13", "min13"],
    },
    Music21ChordType {
        kind: "augmented-major-13th",
        notation: "1,3,#5,7,9,11,13",
        abbreviation: "+M13",
        abbreviations: &["+M13", "augmaj13"],
    },
    Music21ChordType {
        kind: "augmented-dominant-13th",
        notation: "1,3,#5,-7,9,11,13",
        abbreviation: "+13",
        abbreviations: &["+13", "aug13"],
    },
    Music21ChordType {
        kind: "half-diminished-13th",
        notation: "1,-3,-5,-7,9,11,13",
        abbreviation: "ø13",
        abbreviations: &["ø13"],
    },
    Music21ChordType {
        kind: "suspended-second",
        notation: "1,2,5",
        abbreviation: "sus2",
        abbreviations: &["sus2"],
    },
    Music21ChordType {
        kind: "suspended-fourth",
        notation: "1,4,5",
        abbreviation: "sus",
        abbreviations: &["sus", "sus4"],
    },
    Music21ChordType {
        kind: "suspended-fourth-seventh",
        notation: "1,4,5,-7",
        abbreviation: "7sus",
        abbreviations: &["7sus", "7sus4"],
    },
    Music21ChordType {
        kind: "Neapolitan",
        notation: "1,-2,3,-5",
        abbreviation: "N6",
        abbreviations: &["N6"],
    },
    Music21ChordType {
        kind: "Italian",
        notation: "1,#4,-6",
        abbreviation: "It+6",
        abbreviations: &["It+6", "It"],
    },
    Music21ChordType {
        kind: "French",
        notation: "1,2,#4,-6",
        abbreviation: "Fr+6",
        abbreviations: &["Fr+6", "Fr"],
    },
    Music21ChordType {
        kind: "German",
        notation: "1,-3,#4,-6",
        abbreviation: "Gr+6",
        abbreviations: &["Gr+6", "Ger"],
    },
    Music21ChordType {
        kind: "pedal",
        notation: "1",
        abbreviation: "pedal",
        abbreviations: &["pedal"],
    },
    Music21ChordType {
        kind: "power",
        notation: "1,5",
        abbreviation: "power",
        abbreviations: &["power"],
    },
    Music21ChordType {
        kind: "Tristan",
        notation: "1,#4,#6,#9",
        abbreviation: "tristan",
        abbreviations: &["tristan"],
    },
];

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

/// The lead-sheet symbol a chord is written as, as a list so a caller can
/// keep using it where several were once offered: [`ChordSymbolFigure`]
/// written out, or nothing when no kind fits.
pub(crate) fn chord_symbol_spellings(chord: &Chord) -> Vec<String> {
    ChordSymbolFigure::from_chord(chord)
        .map(|figure| figure.to_string())
        .into_iter()
        .collect()
}

/// [`chord_symbol_spellings`] with the root fixed as the pitch of the chord
/// that has the given pitch class, or nothing when no pitch has it.
pub(crate) fn chord_symbol_spellings_with_root(chord: &Chord, root: u8) -> Vec<String> {
    let Some(root) = chord
        .pitches()
        .into_iter()
        .find(|pitch| pitch_class(pitch) == root % 12)
    else {
        return Vec::new();
    };
    ChordSymbolFigure::from_chord_with_root(chord, &root)
        .map(|figure| figure.to_string())
        .into_iter()
        .collect()
}

/// A chord named as a lead-sheet symbol, in the parts music21's
/// `chordSymbolFigureFromChord` writes it from: the root, the kind and its
/// abbreviation, the bass where it is not the root, and the notes the kind
/// does not account for.
///
/// `Display` writes the figure as music21 writes it — `C7`, `E-m7/G-`,
/// `CaddD-` — and [`Self::written_with`] writes it with another abbreviation
/// for the kind.
#[derive(Clone, Debug, PartialEq, Eq)]
#[must_use]
pub struct ChordSymbolFigure {
    /// The root the figure is written on.
    ///
    /// A suspended second in inversion is read as a suspended fourth on its
    /// bass, as music21 reads it, so for that chord this is the bass.
    pub root: String,
    /// music21's name for the kind, `dominant-seventh`.
    pub kind: &'static str,
    /// The abbreviation music21 writes the kind with, `7`.
    pub abbreviation: &'static str,
    /// The bass, where it is not the root.
    pub bass: Option<String>,
    /// The notes of the chord the kind does not account for.
    pub additions: Vec<String>,
    /// The notes the kind expects that the chord lacks. music21 writes these
    /// only beside additions: a chord that leaves out a note of its kind is
    /// still that kind, and says nothing about it.
    pub omissions: Vec<String>,
}

impl ChordSymbolFigure {
    /// The figure of a chord, or `None` for an empty chord, a microtonal one,
    /// or one no kind in music21's table fits.
    pub fn from_chord(chord: &Chord) -> Option<Self> {
        let pitches = chord.pitches();
        let microtonal = pitches
            .iter()
            .any(|pitch| (pitch.ps() - pitch.ps().round()).abs() > FloatType::EPSILON);
        if pitches.is_empty() || microtonal {
            return None;
        }
        let root = chord.root()?.clone();
        if pitches.len() == 1 {
            return Some(Self {
                root: root.name(),
                kind: "pedal",
                abbreviation: "pedal",
                bass: None,
                additions: Vec::new(),
                omissions: Vec::new(),
            });
        }
        let matched = identify_music21_chord_type(&Music21ChordAnalysis::of(chord))?;
        let bass = chord.bass()?.clone();
        let inverted = pitch_class(&bass) != pitch_class(&root);
        let (root, kind, abbreviation, notation) = if inverted && matched.kind == "suspended-second"
        {
            (bass.clone(), "suspended-fourth", "sus", "1,4,5")
        } else {
            (root, matched.kind, matched.abbreviation, matched.notation)
        };
        let bass = (pitch_class(&bass) != pitch_class(&root)).then(|| bass.name());
        let mut perfect = kind_pitch_names(&root, notation).ok()?;
        // music21 reads the figure back through its `ChordSymbol`, which
        // adds a bass the kind does not carry to the notes it sounds, so a
        // bass is never an addition.
        perfect.extend(bass.clone());
        let present: BTreeSet<String> = pitches.iter().map(Pitch::name).collect();
        let (additions, omissions) = if perfect.is_superset(&present) {
            (Vec::new(), Vec::new())
        } else {
            (
                present.difference(&perfect).cloned().collect(),
                perfect.difference(&present).cloned().collect(),
            )
        };
        Some(Self {
            root: root.name(),
            kind,
            abbreviation,
            bass,
            additions,
            omissions,
        })
    }

    /// The figure with the root fixed by the caller rather than inferred,
    /// which is how music21 names the augmented sixths.
    pub fn from_chord_with_root(chord: &Chord, root: &Pitch) -> Option<Self> {
        let mut chord = chord.clone();
        chord.set_root(Some(root.clone()));
        Self::from_chord(&chord)
    }

    /// The figure written with another abbreviation for its kind, which is
    /// how music21 writes it after `changeAbbreviationFor`.
    pub fn written_with(&self, abbreviation: &str) -> String {
        let mut figure = format!("{}{abbreviation}", self.root);
        if let Some(bass) = &self.bass {
            figure.push('/');
            figure.push_str(bass);
        }
        if !self.additions.is_empty() {
            figure.push_str("add");
            figure.push_str(&self.additions.join(","));
            if !self.omissions.is_empty() {
                figure.push_str("omit");
                figure.push_str(&self.omissions.join(","));
            }
        }
        figure
    }
}

impl std::fmt::Display for ChordSymbolFigure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.written_with(self.abbreviation))
    }
}

impl Music21ChordAnalysis {
    /// What music21's `chordSymbolFigureFromChord` reads off a chord before
    /// it looks for a kind, above the root the chord reports.
    fn of(chord: &Chord) -> Self {
        let step = |degree: u8| chord.semitones_from_chord_step(degree);
        Self {
            d3: step(3),
            d5: step(5),
            d7: step(7),
            d9: step(2),
            d11: step(4),
            d13: step(6),
            is_triad: chord.is_triad(),
            is_seventh: chord.is_seventh(),
        }
    }
}
fn identify_music21_chord_type(analysis: &Music21ChordAnalysis) -> Option<Music21FigureMatch> {
    let mut matched = None;

    for chord_type in MUSIC21_CHORD_TYPES {
        let chord_degrees = chord_degrees_for_notation(chord_type.notation)?;
        let is_match = match chord_degrees.len() {
            2 if analysis.is_triad => {
                compare_music21_degrees(&[analysis.d3, analysis.d5], &chord_degrees, &[])
            }
            3 if analysis.is_seventh => compare_music21_degrees(
                &[analysis.d3, analysis.d5, analysis.d7],
                &chord_degrees,
                &[],
            ),
            4 if music21_truthy(analysis.d9)
                && !music21_truthy(analysis.d11)
                && !music21_truthy(analysis.d13) =>
            {
                compare_music21_degrees(
                    &[analysis.d3, analysis.d5, analysis.d7, analysis.d9],
                    &chord_degrees,
                    &[5],
                )
            }
            5 if music21_truthy(analysis.d11) && !music21_truthy(analysis.d13) => {
                compare_music21_degrees(
                    &[
                        analysis.d3,
                        analysis.d5,
                        analysis.d7,
                        analysis.d9,
                        analysis.d11,
                    ],
                    &chord_degrees,
                    &[3, 5],
                )
            }
            6 if music21_truthy(analysis.d13) => compare_music21_degrees(
                &[
                    analysis.d3,
                    analysis.d5,
                    analysis.d7,
                    analysis.d9,
                    analysis.d11,
                    analysis.d13,
                ],
                &chord_degrees,
                &[5, 11, 9],
            ),
            _ => false,
        };

        if is_match {
            matched = Some(Music21FigureMatch {
                kind: chord_type.kind,
                notation: chord_type.notation,
                abbreviation: chord_type.abbreviation,
            });
        }
    }

    if matched.is_some() {
        return matched;
    }

    let mut number_of_matched_degrees = 0;
    for chord_type in MUSIC21_CHORD_TYPES {
        let chord_degrees = chord_degrees_for_notation(chord_type.notation)?;
        let mut degrees = degree_numbers_for_notation(chord_type.notation)?;
        degrees.sort_unstable();
        let to_compare = degrees
            .into_iter()
            .filter(|degree| *degree != 1)
            .map(|degree| analysis_value_for_degree(analysis, degree))
            .collect::<Vec<_>>();

        if compare_music21_degrees(&to_compare, &chord_degrees, &[])
            && number_of_matched_degrees < chord_degrees.len()
        {
            number_of_matched_degrees = chord_degrees.len();
            matched = Some(Music21FigureMatch {
                kind: chord_type.kind,
                notation: chord_type.notation,
                abbreviation: chord_type.abbreviation,
            });
        }
    }

    matched
}

fn compare_music21_degrees(
    in_chord_nums: &[Option<u8>],
    given_chord_nums: &[u8],
    permitted_omissions: &[u8],
) -> bool {
    if given_chord_nums.len() > in_chord_nums.len() {
        return false;
    }

    for (index, expected) in given_chord_nums.iter().enumerate() {
        if in_chord_nums[index] == Some(*expected) {
            continue;
        }

        let (degree, natural) = match index {
            0 => (3, 4),
            1 => (5, 7),
            2 => (7, 11),
            3 => (9, 2),
            4 => (11, 5),
            5 => (13, 9),
            _ => return false,
        };

        if !(permitted_omissions.contains(&degree)
            && *expected == natural
            && in_chord_nums[index].is_none())
        {
            return false;
        }
    }

    true
}

fn music21_truthy(value: Option<u8>) -> bool {
    value.is_some_and(|value| value != 0)
}

fn chord_degrees_for_notation(notation: &str) -> Option<Vec<u8>> {
    notation
        .split(',')
        .filter(|token| *token != "1")
        .map(|token| parse_music21_degree(token).map(|degree| degree.semitone))
        .collect()
}

fn degree_numbers_for_notation(notation: &str) -> Option<Vec<u8>> {
    notation
        .split(',')
        .map(|token| parse_music21_degree(token).map(|degree| degree.degree))
        .collect()
}

fn parse_music21_degree(token: &str) -> Option<Music21Degree> {
    let alteration = token.chars().fold(0_i32, |sum, ch| match ch {
        '#' => sum + 1,
        '-' => sum - 1,
        _ => sum,
    });
    let degree = token
        .chars()
        .filter(char::is_ascii_digit)
        .collect::<String>()
        .parse::<u8>()
        .ok()?;
    let semitone = (base_semitone_for_degree(degree)? + alteration).rem_euclid(12) as u8;

    Some(Music21Degree { degree, semitone })
}

fn base_semitone_for_degree(degree: u8) -> Option<IntegerType> {
    match degree {
        1 => Some(0),
        2 | 9 => Some(2),
        3 => Some(4),
        4 | 11 => Some(5),
        5 => Some(7),
        6 | 13 => Some(9),
        7 => Some(11),
        _ => None,
    }
}

fn analysis_value_for_degree(analysis: &Music21ChordAnalysis, degree: u8) -> Option<u8> {
    match degree {
        2 | 9 => analysis.d9,
        3 => analysis.d3,
        4 | 11 => analysis.d11,
        5 => analysis.d5,
        6 | 13 => analysis.d13,
        7 => analysis.d7,
        _ => None,
    }
}

/// The names of the notes a chord kind's notation stands for above a root:
/// what music21's `ChordSymbol` realizes for the kind alone, and what the
/// figure writer compares a chord against.
fn kind_pitch_names(root: &Pitch, notation: &str) -> Result<BTreeSet<String>> {
    notation_intervals(notation)?
        .iter()
        .map(|(_, name)| Ok(Interval::from_name(name)?.transpose_pitch(root)?.name()))
        .collect()
}

/// The interval above the root each degree of a kind's notation stands for,
/// `1,3,#5,-7` being `P1`, `M3`, `a5` and `m7`: the major-scale interval of
/// the degree, raised or lowered by each `#` or `-` written against it.
fn notation_intervals(notation: &str) -> Result<Vec<(u8, String)>> {
    notation
        .split(',')
        .map(|token| {
            let degree: u8 = token
                .trim_matches(['#', '-'])
                .parse()
                .map_err(|_| Error::Chord(format!("{token} is not a chord degree")))?;
            let alter = token.matches('#').count() as IntegerType
                - token.matches('-').count() as IntegerType;
            let perfect = matches!(degree % 7, 1 | 4 | 5);
            let quality = match (perfect, alter) {
                (true, 0) => "P".to_string(),
                (false, 0) => "M".to_string(),
                (_, raised) if raised > 0 => "a".repeat(raised as usize),
                (true, lowered) => "d".repeat(lowered.unsigned_abs() as usize),
                (false, -1) => "m".to_string(),
                (false, lowered) => "d".repeat((lowered.unsigned_abs() - 1) as usize),
            };
            Ok((degree, format!("{quality}{degree}")))
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

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct Music21PitchModifiers {
    base: String,
    additions: Vec<String>,
    omissions: Vec<String>,
}

fn split_music21_pitch_modifiers(value: &str) -> Music21PitchModifiers {
    let Some(start) = find_music21_modifier_start(value) else {
        return Music21PitchModifiers {
            base: value.to_string(),
            ..Music21PitchModifiers::default()
        };
    };

    let mut parts = Music21PitchModifiers {
        base: value[..start].trim_end().to_string(),
        ..Music21PitchModifiers::default()
    };
    let mut cursor = start;
    while cursor < value.len() {
        let Some(marker) = music21_modifier_at(value, cursor) else {
            cursor += value[cursor..]
                .chars()
                .next()
                .map(char::len_utf8)
                .unwrap_or(1);
            continue;
        };
        let content_start = cursor + marker.len();
        let content_end = find_music21_modifier_start(&value[content_start..])
            .map(|relative| content_start + relative)
            .unwrap_or(value.len());
        let tokens = value[content_start..content_end]
            .split(|ch: char| ch == ',' || ch.is_whitespace())
            .filter(|token| !token.trim().is_empty())
            .map(|token| token.trim().to_string());

        match marker {
            "add" => parts.additions.extend(tokens),
            "omit" => parts.omissions.extend(tokens),
            _ => {}
        }
        cursor = content_end;
    }

    parts
}

fn find_music21_modifier_start(value: &str) -> Option<usize> {
    value
        .char_indices()
        .find_map(|(idx, _)| music21_modifier_at(value, idx).map(|_| idx))
}

fn music21_modifier_at(value: &str, idx: usize) -> Option<&'static str> {
    let rest = value.get(idx..)?;
    let lower = rest.to_ascii_lowercase();
    if lower.starts_with("add") && !matches!(rest.as_bytes().get(3), Some(b'(')) {
        Some("add")
    } else if lower.starts_with("omit") && !matches!(rest.as_bytes().get(4), Some(b'(')) {
        Some("omit")
    } else {
        None
    }
}

fn pitch_name_addition(root: &Pitch, pitch_name: &str) -> Option<ChordAlteration> {
    let pitch = Pitch::from_name(pitch_name).ok()?;
    let degree = pitch_name_degree(root, pitch_name)?;
    let actual = ((pitch_class(&pitch) + 12 - pitch_class(root)) % 12) as IntegerType;
    let base = base_semitone_for_degree(degree)?.rem_euclid(12);
    let mut semitones = actual - base;
    while semitones > 6 {
        semitones -= 12;
    }
    while semitones < -6 {
        semitones += 12;
    }

    Some(ChordAlteration::new(degree, semitones))
}

fn pitch_name_degree(root: &Pitch, pitch_name: &str) -> Option<u8> {
    let pitch = Pitch::from_name(pitch_name).ok()?;
    let generic = (step_num(&pitch) - step_num(root)).rem_euclid(7) + 1;
    Some(match generic as u8 {
        2 => 9,
        4 => 11,
        6 => 13,
        degree => degree,
    })
}

/// Whether the figure names a major seventh over a triad that is not major.
///
/// music21 writes those as `mM7`, `+M9`, `minmaj11`, `augmaj13` and `m#7`.
/// The seventh they name is a semitone above the one the triad's own quality
/// gives — `F#mM7` is `F# A C# E#` where `F#m7` is `F# A C# E` — so it is
/// recorded as an alteration of the seventh. Read on the figure as written,
/// since the `m` and the `M` are the same letter in different cases.
fn major_seventh_over_a_lesser_triad(suffix: &str) -> bool {
    // Longest marker first, so `minmaj7` is not read as `m` followed by
    // `inmaj7`.
    let Some(rest) = ["min", "aug", "dim", "m", "+", "o"]
        .into_iter()
        .find_map(|marker| suffix.strip_prefix(marker))
    else {
        return false;
    };
    let Some(rest) = rest
        .strip_prefix("maj")
        .or_else(|| rest.strip_prefix('M'))
        .or_else(|| rest.strip_prefix('#'))
    else {
        return false;
    };
    ["7", "9", "11", "13"]
        .into_iter()
        .any(|degree| rest.starts_with(degree))
}

fn add_implicit_music21_alterations(suffix: &str, alterations: &mut Vec<ChordAlteration>) {
    let lower = suffix.to_ascii_lowercase();
    if major_seventh_over_a_lesser_triad(suffix)
        && !alterations.iter().any(|alteration| alteration.degree == 7)
    {
        alterations.push(ChordAlteration::new(7, 1));
    }
    if lower.contains("dim5")
        && !alterations
            .iter()
            .any(|alteration| alteration.degree == 5 && alteration.semitones == -1)
    {
        alterations.push(ChordAlteration::new(5, -1));
    }
    if lower.ends_with("7+")
        && !alterations
            .iter()
            .any(|alteration| alteration.degree == 5 && alteration.semitones == 1)
    {
        alterations.push(ChordAlteration::new(5, 1));
    }
}

fn alteration_text(alteration: &ChordAlteration) -> String {
    let sign = if alteration.semitones < 0 { "b" } else { "#" };
    format!(
        "{}{}",
        sign.repeat(alteration.semitones.unsigned_abs() as usize),
        alteration.degree
    )
}

fn parse_quality(suffix: &str, alterations: &[ChordAlteration]) -> ChordQuality {
    let lower = suffix.to_ascii_lowercase();
    let has_flat_five = alterations
        .iter()
        .any(|alteration| alteration.degree == 5 && alteration.semitones == -1);

    if suffix.starts_with('\u{00f8}') {
        ChordQuality::HalfDiminished
    } else if lower.contains("sus2") {
        ChordQuality::Suspended2
    } else if lower.contains("sus") {
        ChordQuality::Suspended4
    } else if lower.starts_with("maj") || suffix.starts_with('M') {
        ChordQuality::Major
    } else if lower.starts_with("min") || lower.starts_with('m') {
        if has_flat_five && lower.contains('7') {
            ChordQuality::HalfDiminished
        } else {
            ChordQuality::Minor
        }
    } else if lower.starts_with("dim") || lower.starts_with('o') {
        ChordQuality::Diminished
    } else if lower.starts_with("aug") || lower.starts_with('+') {
        ChordQuality::Augmented
    } else if lower.starts_with('5') || lower.starts_with("power") {
        ChordQuality::Power
    } else if lower.starts_with("pedal") {
        ChordQuality::Pedal
    } else if lower.starts_with("dom")
        || lower.starts_with('7')
        || lower.starts_with('9')
        || lower.starts_with("11")
        || lower.starts_with("13")
    {
        ChordQuality::Dominant
    } else {
        ChordQuality::Major
    }
}

fn parse_extensions(suffix: &str, alterations: &[ChordAlteration]) -> Vec<u8> {
    let mut extensions = Vec::new();
    let bytes = suffix.as_bytes();
    let mut idx = 0;
    while idx < bytes.len() {
        let byte = bytes[idx];
        if byte.is_ascii_digit()
            && idx
                .checked_sub(1)
                .is_none_or(|prev| !matches!(bytes[prev] as char, '#' | 'b' | '-'))
        {
            let start = idx;
            while idx < bytes.len() && bytes[idx].is_ascii_digit() {
                idx += 1;
            }
            if let Ok(degree) = suffix[start..idx].parse::<u8>()
                && matches!(degree, 6 | 7 | 9 | 11 | 13)
                && !extensions.contains(&degree)
            {
                extensions.push(degree);
            }
        } else {
            idx += 1;
        }
    }

    for alteration in alterations {
        if alteration.degree > 5 && !extensions.contains(&alteration.degree) {
            extensions.push(alteration.degree);
        }
    }

    extensions.sort_unstable();
    extensions
}

fn strip_addition_groups(suffix: &str) -> String {
    let lower = suffix.to_ascii_lowercase();
    let mut stripped = String::with_capacity(suffix.len());
    let mut cursor = 0;

    while let Some(relative_start) = lower[cursor..].find("add(") {
        let start = cursor + relative_start;
        let content_start = start + "add(".len();
        let Some(relative_end) = suffix[content_start..].find(')') else {
            break;
        };

        stripped.push_str(&suffix[cursor..start]);
        cursor = content_start + relative_end + 1;
    }

    stripped.push_str(&suffix[cursor..]);
    stripped
}

fn parse_additions(suffix: &str) -> Vec<ChordAlteration> {
    let lower = suffix.to_ascii_lowercase();
    let mut additions = Vec::new();
    let mut cursor = 0;

    while let Some(relative_start) = lower[cursor..].find("add(") {
        let content_start = cursor + relative_start + "add(".len();
        let Some(relative_end) = suffix[content_start..].find(')') else {
            break;
        };
        let content_end = content_start + relative_end;

        for token in
            suffix[content_start..content_end].split(|ch: char| ch == ',' || ch.is_whitespace())
        {
            if let Some(addition) = parse_addition_token(token) {
                additions.push(addition);
            }
        }

        cursor = content_end + 1;
    }

    additions
}

fn parse_omissions(suffix: &str) -> Vec<u8> {
    let lower = suffix.to_ascii_lowercase();
    let bytes = lower.as_bytes();
    let mut omissions = Vec::new();
    let mut cursor = 0;

    while cursor < bytes.len() {
        let marker_len = if bytes[cursor..].starts_with(b"omit") {
            4
        } else if bytes[cursor..].starts_with(b"no") {
            2
        } else {
            cursor += 1;
            continue;
        };

        cursor += marker_len;
        while cursor < bytes.len() && (bytes[cursor].is_ascii_whitespace() || bytes[cursor] == b'(')
        {
            cursor += 1;
        }

        let degree_start = cursor;
        while cursor < bytes.len() && bytes[cursor].is_ascii_digit() {
            cursor += 1;
        }
        if degree_start == cursor {
            continue;
        }

        if let Ok(degree) = std::str::from_utf8(&bytes[degree_start..cursor])
            .unwrap_or_default()
            .parse::<u8>()
            && !omissions.contains(&degree)
        {
            omissions.push(degree);
        }
    }

    omissions
}

fn parse_addition_token(token: &str) -> Option<ChordAlteration> {
    let token = token.trim();
    if token.is_empty() {
        return None;
    }

    let (semitones, degree) = match token.as_bytes()[0] as char {
        '#' => (1, &token[1..]),
        'b' | '-' => (-1, &token[1..]),
        _ => (0, token),
    };

    degree
        .parse::<u8>()
        .ok()
        .map(|degree| ChordAlteration::new(degree, semitones))
}

fn parse_alterations(suffix: &str) -> Vec<ChordAlteration> {
    let bytes = suffix.as_bytes();
    let mut alterations = Vec::new();
    let mut idx = 0;
    while idx < bytes.len() {
        let semitones = match bytes[idx] as char {
            '#' => 1,
            'b' | '-' => -1,
            _ => {
                idx += 1;
                continue;
            }
        };
        idx += 1;
        let start = idx;
        while idx < bytes.len() && bytes[idx].is_ascii_digit() {
            idx += 1;
        }
        if start == idx {
            continue;
        }
        if let Ok(degree) = suffix[start..idx].parse::<u8>() {
            alterations.push(ChordAlteration::new(degree, semitones));
        }
    }
    alterations
}

fn parse_pitch_only(value: &str) -> Result<Pitch> {
    let (name, rest) = parse_pitch_prefix(value)?;
    if !rest.is_empty() {
        return Err(Error::Chord(format!("invalid slash bass {value:?}")));
    }
    Pitch::from_name(name)
}

fn parse_pitch_prefix(value: &str) -> Result<(String, &str)> {
    let mut chars = value.char_indices();
    let Some((_, first)) = chars.next() else {
        return Err(Error::Chord("missing pitch name".to_string()));
    };

    if !matches!(first.to_ascii_uppercase(), 'A'..='G') {
        return Err(Error::Chord(format!("invalid pitch name in {value:?}")));
    }

    let mut end = first.len_utf8();
    let mut name = first.to_ascii_uppercase().to_string();
    for (idx, ch) in chars {
        match ch {
            '#' => {
                name.push('#');
                end = idx + ch.len_utf8();
            }
            'b' | '-' => {
                name.push('-');
                end = idx + ch.len_utf8();
            }
            _ => break,
        }
    }

    Ok((name, &value[end..]))
}

fn default_extension_interval(degree: u8) -> &'static str {
    match degree {
        6 => "M6",
        9 => "M9",
        11 => "P11",
        13 => "M13",
        _ => "P1",
    }
}

fn altered_interval(alteration: &ChordAlteration) -> Result<(u8, &'static str)> {
    match (alteration.degree, alteration.semitones) {
        (5, -1) => Ok((5, "d5")),
        (5, 1) => Ok((5, "a5")),
        // The seventh a figure raises is the major seventh, since the one it
        // is raised from is the minor seventh every non-major kind carries.
        (7, -1) => Ok((7, "d7")),
        (7, 1) => Ok((7, "M7")),
        (9, -1) => Ok((9, "m9")),
        (9, 1) => Ok((9, "a9")),
        (11, 1) => Ok((11, "a11")),
        (13, -1) => Ok((13, "m13")),
        (13, 1) => Ok((13, "a13")),
        _ => Err(Error::Chord(format!(
            "unsupported chord-symbol alteration {alteration:?}"
        ))),
    }
}

fn added_interval(addition: &ChordAlteration) -> Result<(u8, &'static str)> {
    let degree = match addition.degree {
        2 => 9,
        4 => 11,
        6 => 13,
        degree => degree,
    };

    match (degree, addition.semitones) {
        (3, -1) => Ok((degree, "m3")),
        (3, 0) => Ok((degree, "M3")),
        (3, 1) => Ok((degree, "a3")),
        (5, -1) => Ok((degree, "d5")),
        (5, 0) => Ok((degree, "P5")),
        (5, 1) => Ok((degree, "a5")),
        (7, -1) => Ok((degree, "m7")),
        (7, 0) => Ok((degree, "M7")),
        (9, -1) => Ok((degree, "m9")),
        (9, 0) => Ok((degree, "M9")),
        (9, 1) => Ok((degree, "a9")),
        (11, -1) => Ok((degree, "d11")),
        (11, 0) => Ok((degree, "P11")),
        (11, 1) => Ok((degree, "a11")),
        (13, -1) => Ok((degree, "m13")),
        (13, 0) => Ok((degree, "M13")),
        (13, 1) => Ok((degree, "a13")),
        _ => Err(Error::Chord(format!(
            "unsupported chord-symbol added tone {addition:?}"
        ))),
    }
}

/// Names a chord as a lead-sheet symbol: music21's
/// `chordSymbolFigureFromChord`, so `C E G B-` is `C7`, `E G C` is `C/E`
/// and a lone `C` is `Cpedal`. Notes the kind cannot account for are listed
/// after `add`, and beside them the notes the kind expects but the chord
/// lacks after `omit`, as music21 writes them. `None` when no kind fits,
/// where music21 returns the sentence "Chord Symbol Cannot Be Identified";
/// an empty chord gives an empty string. The parts the figure is written
/// from are [`ChordSymbolFigure`].
pub fn chord_symbol_figure_from_chord(chord: &Chord) -> Result<Option<String>> {
    if chord.notes().is_empty() {
        return Ok(Some(String::new()));
    }
    Ok(ChordSymbolFigure::from_chord(chord).map(|figure| figure.to_string()))
}

/// The kind of chord a figure is written with: what music21 answers beside
/// the figure when `chordSymbolFigureFromChord` is asked to include the chord
/// type, so `C E G` is `major` and a lone `C` is `pedal`.
///
/// `None` for an empty chord, which has no figure either, and for a chord no
/// kind in music21's table fits.
#[must_use]
pub fn chord_symbol_kind_from_chord(chord: &Chord) -> Option<&'static str> {
    ChordSymbolFigure::from_chord(chord).map(|figure| figure.kind)
}
/// A [`ChordSymbol`] read off a chord: music21's `chordSymbolFromChord`,
/// [`chord_symbol_figure_from_chord`] parsed back. `None` when no kind fits.
pub fn chord_symbol_from_chord(chord: &Chord) -> Result<Option<ChordSymbol>> {
    match chord_symbol_figure_from_chord(chord)? {
        Some(figure) if !figure.is_empty() => Ok(Some(ChordSymbol::parse(figure)?)),
        _ => Ok(None),
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
