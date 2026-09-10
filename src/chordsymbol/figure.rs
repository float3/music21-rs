//! Naming a chord as a lead-sheet symbol, the way music21's
//! `chordSymbolFigureFromChord` names it.

use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Music21FigureMatch {
    kind: &'static str,
    notation: &'static str,
    abbreviation: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Music21ChordAnalysis {
    d3: Option<u8>,
    d5: Option<u8>,
    d7: Option<u8>,
    d9: Option<u8>,
    d11: Option<u8>,
    d13: Option<u8>,
    is_triad: bool,
    is_seventh: bool,
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
                figure.push_str(",omit");
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
pub(super) fn identify_music21_chord_type(
    analysis: &Music21ChordAnalysis,
) -> Option<Music21FigureMatch> {
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

pub(super) fn compare_music21_degrees(
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

pub(super) fn music21_truthy(value: Option<u8>) -> bool {
    value.is_some_and(|value| value != 0)
}

pub(super) fn analysis_value_for_degree(analysis: &Music21ChordAnalysis, degree: u8) -> Option<u8> {
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
