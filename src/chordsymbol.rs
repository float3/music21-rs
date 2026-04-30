use std::str::FromStr;

use crate::{
    chord::Chord,
    defaults::{FloatType, IntegerType},
    error::{Error, Result},
    interval::Interval,
    pitch::Pitch,
};
use std::collections::{BTreeMap, BTreeSet};

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
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Music21ChordType {
    kind: &'static str,
    notation: &'static str,
    abbreviation: &'static str,
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
    },
    Music21ChordType {
        kind: "minor",
        notation: "1,-3,5",
        abbreviation: "m",
    },
    Music21ChordType {
        kind: "augmented",
        notation: "1,3,#5",
        abbreviation: "+",
    },
    Music21ChordType {
        kind: "diminished",
        notation: "1,-3,-5",
        abbreviation: "dim",
    },
    Music21ChordType {
        kind: "dominant-seventh",
        notation: "1,3,5,-7",
        abbreviation: "7",
    },
    Music21ChordType {
        kind: "major-seventh",
        notation: "1,3,5,7",
        abbreviation: "maj7",
    },
    Music21ChordType {
        kind: "minor-major-seventh",
        notation: "1,-3,5,7",
        abbreviation: "mM7",
    },
    Music21ChordType {
        kind: "minor-seventh",
        notation: "1,-3,5,-7",
        abbreviation: "m7",
    },
    Music21ChordType {
        kind: "augmented-major-seventh",
        notation: "1,3,#5,7",
        abbreviation: "+M7",
    },
    Music21ChordType {
        kind: "augmented-seventh",
        notation: "1,3,#5,-7",
        abbreviation: "7+",
    },
    Music21ChordType {
        kind: "half-diminished-seventh",
        notation: "1,-3,-5,-7",
        abbreviation: "\u{00f8}7",
    },
    Music21ChordType {
        kind: "diminished-seventh",
        notation: "1,-3,-5,--7",
        abbreviation: "o7",
    },
    Music21ChordType {
        kind: "seventh-flat-five",
        notation: "1,3,-5,-7",
        abbreviation: "dom7dim5",
    },
    Music21ChordType {
        kind: "major-sixth",
        notation: "1,3,5,6",
        abbreviation: "6",
    },
    Music21ChordType {
        kind: "minor-sixth",
        notation: "1,-3,5,6",
        abbreviation: "m6",
    },
    Music21ChordType {
        kind: "major-ninth",
        notation: "1,3,5,7,9",
        abbreviation: "M9",
    },
    Music21ChordType {
        kind: "dominant-ninth",
        notation: "1,3,5,-7,9",
        abbreviation: "9",
    },
    Music21ChordType {
        kind: "minor-major-ninth",
        notation: "1,-3,5,7,9",
        abbreviation: "mM9",
    },
    Music21ChordType {
        kind: "minor-ninth",
        notation: "1,-3,5,-7,9",
        abbreviation: "m9",
    },
    Music21ChordType {
        kind: "augmented-major-ninth",
        notation: "1,3,#5,7,9",
        abbreviation: "+M9",
    },
    Music21ChordType {
        kind: "augmented-dominant-ninth",
        notation: "1,3,#5,-7,9",
        abbreviation: "9#5",
    },
    Music21ChordType {
        kind: "half-diminished-ninth",
        notation: "1,-3,-5,-7,9",
        abbreviation: "\u{00f8}9",
    },
    Music21ChordType {
        kind: "half-diminished-minor-ninth",
        notation: "1,-3,-5,-7,-9",
        abbreviation: "\u{00f8}b9",
    },
    Music21ChordType {
        kind: "diminished-ninth",
        notation: "1,-3,-5,--7,9",
        abbreviation: "o9",
    },
    Music21ChordType {
        kind: "diminished-minor-ninth",
        notation: "1,-3,-5,--7,-9",
        abbreviation: "ob9",
    },
    Music21ChordType {
        kind: "dominant-11th",
        notation: "1,3,5,-7,9,11",
        abbreviation: "11",
    },
    Music21ChordType {
        kind: "major-11th",
        notation: "1,3,5,7,9,11",
        abbreviation: "M11",
    },
    Music21ChordType {
        kind: "minor-major-11th",
        notation: "1,-3,5,7,9,11",
        abbreviation: "mM11",
    },
    Music21ChordType {
        kind: "minor-11th",
        notation: "1,-3,5,-7,9,11",
        abbreviation: "m11",
    },
    Music21ChordType {
        kind: "augmented-major-11th",
        notation: "1,3,#5,7,9,11",
        abbreviation: "+M11",
    },
    Music21ChordType {
        kind: "augmented-11th",
        notation: "1,3,#5,-7,9,11",
        abbreviation: "+11",
    },
    Music21ChordType {
        kind: "half-diminished-11th",
        notation: "1,-3,-5,-7,9,11",
        abbreviation: "\u{00f8}11",
    },
    Music21ChordType {
        kind: "diminished-11th",
        notation: "1,-3,-5,--7,9,11",
        abbreviation: "o11",
    },
    Music21ChordType {
        kind: "major-13th",
        notation: "1,3,5,7,9,11,13",
        abbreviation: "M13",
    },
    Music21ChordType {
        kind: "dominant-13th",
        notation: "1,3,5,-7,9,11,13",
        abbreviation: "13",
    },
    Music21ChordType {
        kind: "minor-major-13th",
        notation: "1,-3,5,7,9,11,13",
        abbreviation: "mM13",
    },
    Music21ChordType {
        kind: "minor-13th",
        notation: "1,-3,5,-7,9,11,13",
        abbreviation: "m13",
    },
    Music21ChordType {
        kind: "augmented-major-13th",
        notation: "1,3,#5,7,9,11,13",
        abbreviation: "+M13",
    },
    Music21ChordType {
        kind: "augmented-dominant-13th",
        notation: "1,3,#5,-7,9,11,13",
        abbreviation: "+13",
    },
    Music21ChordType {
        kind: "half-diminished-13th",
        notation: "1,-3,-5,-7,9,11,13",
        abbreviation: "\u{00f8}13",
    },
    Music21ChordType {
        kind: "suspended-second",
        notation: "1,2,5",
        abbreviation: "sus2",
    },
    Music21ChordType {
        kind: "suspended-fourth",
        notation: "1,4,5",
        abbreviation: "sus",
    },
    Music21ChordType {
        kind: "suspended-fourth-seventh",
        notation: "1,4,5,-7",
        abbreviation: "7sus",
    },
    Music21ChordType {
        kind: "Neapolitan",
        notation: "1,-2,3,-5",
        abbreviation: "N6",
    },
    Music21ChordType {
        kind: "Italian",
        notation: "1,#4,-6",
        abbreviation: "It+6",
    },
    Music21ChordType {
        kind: "French",
        notation: "1,2,#4,-6",
        abbreviation: "Fr+6",
    },
    Music21ChordType {
        kind: "German",
        notation: "1,-3,#4,-6",
        abbreviation: "Gr+6",
    },
    Music21ChordType {
        kind: "pedal",
        notation: "1",
        abbreviation: "pedal",
    },
    Music21ChordType {
        kind: "power",
        notation: "1,5",
        abbreviation: "power",
    },
    Music21ChordType {
        kind: "Tristan",
        notation: "1,#4,#6,#9",
        abbreviation: "tristan",
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

        Ok(Self {
            figure: trimmed.to_string(),
            root,
            bass,
            quality,
            extensions,
            alterations,
            omissions,
            additions,
        })
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

    /// Realizes the chord symbol as a [`Chord`].
    pub fn to_chord(&self) -> Result<Chord> {
        let mut interval_names = self.base_intervals();

        for extension in [6, 9, 11, 13] {
            if self.extensions.contains(&extension)
                && !self.alterations.iter().any(|alt| alt.degree == extension)
            {
                interval_names.push(default_extension_interval(extension));
            }
        }

        for alteration in &self.alterations {
            if alteration.degree == 5 && matches!(self.quality, ChordQuality::HalfDiminished) {
                continue;
            }
            if alteration.degree == 5 {
                continue;
            }
            interval_names.push(altered_interval(alteration)?);
        }

        for addition in &self.additions {
            interval_names.push(added_interval(addition)?);
        }

        interval_names.sort_unstable_by_key(|name| interval_sort_key(name));
        interval_names.dedup();

        let mut pitches = interval_names
            .into_iter()
            .map(|name| Interval::from_name(name)?.transpose_pitch(&self.root))
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

    fn base_intervals(&self) -> Vec<&'static str> {
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
        };

        intervals
            .into_iter()
            .filter_map(|(degree, interval)| {
                (!self.omissions.contains(&degree)).then_some(interval)
            })
            .collect()
    }
}

/// Returns the music21 chord-symbol figure for a chord, when identified.
///
/// This ports music21's `harmony.chordSymbolFigureFromChord` matching order and
/// spelling conventions. Music21's "Chord Symbol Cannot Be Identified" result
/// is represented by an empty list so callers can keep using `Option<String>`.
pub(crate) fn chord_symbol_spellings(chord: &Chord) -> Vec<String> {
    chord_symbol_spellings_for_root(chord, None)
}

pub(crate) fn chord_symbol_spellings_with_root(chord: &Chord, root: u8) -> Vec<String> {
    chord_symbol_spellings_for_root(chord, Some(root % 12))
}

fn chord_symbol_spellings_for_root(chord: &Chord, explicit_root: Option<u8>) -> Vec<String> {
    music21_chord_symbol_figure(chord, explicit_root)
        .into_iter()
        .collect()
}

fn music21_chord_symbol_figure(chord: &Chord, explicit_root: Option<u8>) -> Option<String> {
    let pitches = chord.pitches();
    if pitches.iter().any(|pitch| {
        let ps = pitch.ps();
        (ps - ps.round()).abs() > FloatType::EPSILON
    }) {
        return None;
    }

    if pitches.is_empty() {
        return None;
    }

    let mut root_pitch = if let Some(root) = explicit_root {
        pitches
            .iter()
            .find(|pitch| pitch_class(pitch) == root)
            .cloned()?
    } else {
        find_root_pitch(&pitches).cloned()?
    };

    if pitches.len() == 1 {
        return Some(format!("{}pedal", root_pitch.name()));
    }

    let analysis = Music21ChordAnalysis::new(&pitches, &root_pitch);
    let matched = identify_music21_chord_type(&analysis)?;
    let bass_pitch = bass_pitch(&pitches)?;
    let mut notation = matched.notation;
    let mut abbreviation = matched.abbreviation;

    if pitch_class(bass_pitch) != pitch_class(&root_pitch)
        && matched.kind == "suspended-second"
        && matched.abbreviation == "sus2"
    {
        root_pitch = bass_pitch.clone();
        notation = "1,4,5";
        abbreviation = "sus";
    }

    let mut figure = format!("{}{}", root_pitch.name(), abbreviation);
    if pitch_class(bass_pitch) != pitch_class(&root_pitch) {
        figure.push('/');
        figure.push_str(&bass_pitch.name());
    }

    let perfect = perfect_pitch_names(&root_pitch, notation)?;
    let in_pitches = pitches
        .iter()
        .map(Pitch::name)
        .collect::<BTreeSet<String>>();

    if !perfect.is_superset(&in_pitches) {
        let additions = in_pitches.difference(&perfect).cloned().collect::<Vec<_>>();
        let subtractions = perfect.difference(&in_pitches).cloned().collect::<Vec<_>>();

        if !additions.is_empty() {
            figure.push_str("add");
            figure.push_str(&additions.join(","));
        }
        if !subtractions.is_empty() {
            figure.push_str("omit");
            figure.push_str(&subtractions.join(","));
        }
    }

    Some(figure)
}

impl Music21ChordAnalysis {
    fn new(pitches: &[Pitch], root_pitch: &Pitch) -> Self {
        let d3 = semitones_from_chord_step(pitches, root_pitch, 3);
        let d5 = semitones_from_chord_step(pitches, root_pitch, 5);
        let d7 = semitones_from_chord_step(pitches, root_pitch, 7);
        let d9 = semitones_from_chord_step(pitches, root_pitch, 2);
        let d11 = semitones_from_chord_step(pitches, root_pitch, 4);
        let d13 = semitones_from_chord_step(pitches, root_pitch, 6);
        let unique_pitch_names = pitches
            .iter()
            .map(Pitch::name)
            .collect::<BTreeSet<String>>();

        Self {
            d3,
            d5,
            d7,
            d9,
            d11,
            d13,
            is_triad: unique_pitch_names.len() == 3 && d3.is_some() && d5.is_some(),
            is_seventh: unique_pitch_names.len() == 4
                && d3.is_some()
                && d5.is_some()
                && d7.is_some(),
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

fn semitones_from_chord_step(pitches: &[Pitch], root_pitch: &Pitch, chord_step: u8) -> Option<u8> {
    let root_step = step_num(root_pitch);
    let root_pc = pitch_class(root_pitch);

    pitches.iter().find_map(|pitch| {
        let generic_interval = (step_num(pitch) - root_step).rem_euclid(7) + 1;
        if generic_interval == chord_step as IntegerType {
            Some((pitch_class(pitch) + 12 - root_pc) % 12)
        } else {
            None
        }
    })
}

fn perfect_pitch_names(root_pitch: &Pitch, notation: &str) -> Option<BTreeSet<String>> {
    let mut pitch_names = BTreeSet::new();
    pitch_names.insert(root_pitch.name());
    for token in notation.split(',').filter(|token| *token != "1") {
        let degree = parse_music21_degree(token)?;
        pitch_names.insert(pitch_name_for_music21_degree(
            root_pitch,
            degree.degree,
            degree.semitone,
        )?);
    }
    Some(pitch_names)
}

fn pitch_name_for_music21_degree(root_pitch: &Pitch, degree: u8, semitone: u8) -> Option<String> {
    const LETTERS: [char; 7] = ['C', 'D', 'E', 'F', 'G', 'A', 'B'];
    const NATURAL_PCS: [IntegerType; 7] = [0, 2, 4, 5, 7, 9, 11];

    let root_letter = root_pitch.name().chars().next()?.to_ascii_uppercase();
    let root_index = LETTERS.iter().position(|letter| *letter == root_letter)?;
    let target_index = (root_index + (degree.saturating_sub(1) as usize % 7)) % 7;
    let desired_pc =
        ((pitch_class(root_pitch) as IntegerType) + semitone as IntegerType).rem_euclid(12);
    let mut accidental = desired_pc - NATURAL_PCS[target_index];
    while accidental > 6 {
        accidental -= 12;
    }
    while accidental < -6 {
        accidental += 12;
    }

    let mut name = LETTERS[target_index].to_string();
    if accidental > 0 {
        name.push_str(&"#".repeat(accidental as usize));
    } else if accidental < 0 {
        name.push_str(&"-".repeat((-accidental) as usize));
    }
    Some(name)
}

fn find_root_pitch(pitches: &[Pitch]) -> Option<&Pitch> {
    let mut non_duplicating_pitches = Vec::new();
    let mut seen_steps = BTreeSet::new();
    for pitch in pitches {
        if seen_steps.insert(step_num(pitch)) {
            non_duplicating_pitches.push(pitch);
        }
    }

    match non_duplicating_pitches.len() {
        0 => return None,
        1 => return pitches.first(),
        7 => return bass_pitch(pitches),
        _ => {}
    }

    let mut step_nums_to_pitches = BTreeMap::new();
    for pitch in &non_duplicating_pitches {
        step_nums_to_pitches.insert(step_num(pitch), *pitch);
    }
    let step_nums = step_nums_to_pitches.keys().copied().collect::<Vec<_>>();

    for start_index in 0..step_nums.len() {
        let mut all_are_thirds = true;
        let this_step_num = step_nums[start_index];
        let mut last_step_num = this_step_num;
        for end_index in (start_index + 1)..(start_index + step_nums.len()) {
            let end_step_num = step_nums[end_index % step_nums.len()];
            if !matches!(end_step_num - last_step_num, 2 | -5) {
                all_are_thirds = false;
                break;
            }
            last_step_num = end_step_num;
        }
        if all_are_thirds {
            return step_nums_to_pitches.get(&this_step_num).copied();
        }
    }

    let ordered_chord_steps = [3, 5, 7, 2, 4, 6];
    let mut best_pitch = non_duplicating_pitches[0];
    let mut best_score = FloatType::NEG_INFINITY;

    for pitch in non_duplicating_pitches {
        let this_step_num = step_num(pitch);
        let mut score = 0.0;
        for (root_index, chord_step_test) in ordered_chord_steps.iter().enumerate() {
            let target = (this_step_num + chord_step_test - 1).rem_euclid(7);
            if step_nums_to_pitches.contains_key(&target) {
                score += 1.0 / (root_index as FloatType + 6.0);
            }
        }
        if score > best_score {
            best_score = score;
            best_pitch = pitch;
        }
    }

    Some(best_pitch)
}

fn bass_pitch(pitches: &[Pitch]) -> Option<&Pitch> {
    pitches.iter().min_by(|left, right| {
        left.ps()
            .partial_cmp(&right.ps())
            .unwrap_or(std::cmp::Ordering::Equal)
    })
}

fn step_num(pitch: &Pitch) -> IntegerType {
    pitch.step().step_to_dnn_offset() - 1
}

fn pitch_class(pitch: &Pitch) -> u8 {
    (pitch.ps().round() as IntegerType).rem_euclid(12) as u8
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

fn add_implicit_music21_alterations(suffix: &str, alterations: &mut Vec<ChordAlteration>) {
    let lower = suffix.to_ascii_lowercase();
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
    } else if lower.starts_with('5') {
        ChordQuality::Power
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

fn altered_interval(alteration: &ChordAlteration) -> Result<&'static str> {
    match (alteration.degree, alteration.semitones) {
        (5, -1) => Ok("d5"),
        (5, 1) => Ok("a5"),
        (9, -1) => Ok("m9"),
        (9, 1) => Ok("a9"),
        (11, 1) => Ok("a11"),
        (13, -1) => Ok("m13"),
        (13, 1) => Ok("a13"),
        _ => Err(Error::Chord(format!(
            "unsupported chord-symbol alteration {alteration:?}"
        ))),
    }
}

fn added_interval(addition: &ChordAlteration) -> Result<&'static str> {
    let degree = match addition.degree {
        2 => 9,
        4 => 11,
        6 => 13,
        degree => degree,
    };

    match (degree, addition.semitones) {
        (3, -1) => Ok("m3"),
        (3, 0) => Ok("M3"),
        (3, 1) => Ok("a3"),
        (5, -1) => Ok("d5"),
        (5, 0) => Ok("P5"),
        (5, 1) => Ok("a5"),
        (7, -1) => Ok("m7"),
        (7, 0) => Ok("M7"),
        (9, -1) => Ok("m9"),
        (9, 0) => Ok("M9"),
        (9, 1) => Ok("a9"),
        (11, -1) => Ok("d11"),
        (11, 0) => Ok("P11"),
        (11, 1) => Ok("a11"),
        (13, -1) => Ok("m13"),
        (13, 0) => Ok("M13"),
        (13, 1) => Ok("a13"),
        _ => Err(Error::Chord(format!(
            "unsupported chord-symbol added tone {addition:?}"
        ))),
    }
}

fn interval_sort_key(name: &str) -> u8 {
    name.chars()
        .filter(|ch| ch.is_ascii_digit())
        .collect::<String>()
        .parse::<u8>()
        .unwrap_or(1)
}

#[cfg(test)]
mod tests {
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
