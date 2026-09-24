//! How a dominant seventh, a diminished seventh or an augmented sixth
//! resolves: music21's `figuredBass.resolution`.
//!
//! Each function takes a possibility spelling one of those chords -- a pitch
//! per part, highest part first -- and moves every pitch by the step its
//! place in the chord calls for, so the possibility it answers is the chord
//! it resolves to, voiced part for part. A pitch the rule says nothing about
//! stays where it is.
//!
//! The chord's notes are worked out from the possibility unless the caller
//! hands them over as a [`ChordInfo`], which a realizer does, since it
//! resolves many voicings of one chord.

use std::sync::LazyLock;

use crate::chord::Chord;
use crate::error::{Error, Result};
use crate::interval::Interval;
use crate::pitch::Pitch;

/// A chord's bass, root, third, fifth and seventh, whichever it has: what a
/// resolution is worked out from.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ChordInfo {
    /// The chord's bass.
    pub bass: Option<Pitch>,
    /// Its root.
    pub root: Option<Pitch>,
    /// Its third.
    pub third: Option<Pitch>,
    /// Its fifth.
    pub fifth: Option<Pitch>,
    /// Its seventh.
    pub seventh: Option<Pitch>,
}

impl ChordInfo {
    /// The notes of `chord`, where it has them.
    pub fn of(chord: &Chord) -> Self {
        Self {
            bass: chord.bass().cloned(),
            root: chord.root().cloned(),
            third: chord.chord_step(3).cloned(),
            fifth: chord.chord_step(5).cloned(),
            seventh: chord.chord_step(7).cloned(),
        }
    }
}

/// The three augmented sixths that resolve by a rule of their own. An
/// Italian sixth has no fourth note to place, and is resolved as any chord
/// is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AugmentedSixth {
    /// French: music21's type 1.
    French,
    /// German: music21's type 2.
    German,
    /// Swiss: music21's type 3.
    Swiss,
}

impl AugmentedSixth {
    /// The kind of augmented sixth `chord` is.
    ///
    /// # Errors
    ///
    /// A chord that is no augmented sixth, or an Italian one.
    pub fn of(chord: &Chord) -> Result<Self> {
        if !chord.is_augmented_sixth(false) {
            return Err(Error::FiguredBass(
                "Possibility is not an augmented sixth chord.".to_string(),
            ));
        }
        if chord.is_italian_augmented_sixth(false, false) {
            return Err(Error::FiguredBass(
                "Italian augmented sixth resolution not supported in this method.".to_string(),
            ));
        }
        if chord.is_french_augmented_sixth(false) {
            Ok(Self::French)
        } else if chord.is_german_augmented_sixth(false) {
            Ok(Self::German)
        } else if chord.is_swiss_augmented_sixth(false) {
            Ok(Self::Swiss)
        } else {
            Err(Error::FiguredBass("Unknown augSixthType: None".to_string()))
        }
    }

    /// music21's number for the kind: 1, 2 or 3.
    pub fn number(self) -> u8 {
        match self {
            Self::French => 1,
            Self::German => 2,
            Self::Swiss => 3,
        }
    }

    /// The kind music21 numbers so.
    pub fn from_number(number: u8) -> Option<Self> {
        match number {
            1 => Some(Self::French),
            2 => Some(Self::German),
            3 => Some(Self::Swiss),
            _ => None,
        }
    }
}

/// Every step a resolution moves a pitch by, read once.
static STEPS: LazyLock<Vec<(&'static str, Interval)>> = LazyLock::new(|| {
    ["P1", "m2", "-m2", "M2", "-M2", "A1", "d1", "d2", "P4"]
        .into_iter()
        .map(|name| (name, Interval::from_name(name).expect("a resolution step")))
        .collect()
});

fn step(name: &str) -> &'static Interval {
    STEPS
        .iter()
        .find(|(known, _)| *known == name)
        .map(|(_, interval)| interval)
        .expect("every resolution step is in the table")
}

/// A rule of a resolution: which pitches it moves, and by what.
type Rule<'a> = (Box<dyn Fn(&Pitch) -> bool + 'a>, &'static str);

/// Moves each pitch of `possibility` by the step of the first rule it
/// answers to, leaving it where it is when it answers to none.
fn resolve(possibility: &[Pitch], rules: &[Rule<'_>]) -> Result<Vec<Pitch>> {
    possibility
        .iter()
        .map(|pitch| {
            let name = rules
                .iter()
                .find(|(applies, _)| applies(pitch))
                .map_or("P1", |(_, step)| *step);
            pitch.transpose(step(name))
        })
        .collect()
}

/// Whether `pitch` is spelled as `note`, where there is such a note.
fn named<'a>(note: Option<&'a Pitch>) -> impl Fn(&Pitch) -> bool + 'a {
    move |pitch| note.is_some_and(|note| pitch.name() == note.name())
}

/// Moves each named note of `possibility` by its step, and the rest nowhere.
fn move_named(
    possibility: &[Pitch],
    moves: &[(Option<&Pitch>, &'static str)],
) -> Result<Vec<Pitch>> {
    let rules: Vec<Rule<'_>> = moves
        .iter()
        .map(|&(note, step)| -> Rule<'_> { (Box::new(named(note)), step) })
        .collect();
    resolve(possibility, &rules)
}

/// The chord's notes as a resolution of `possibility` needs them, read off
/// the possibility where the caller has not given them, after `check` has
/// said the chord is of the right kind.
fn info_or_read(
    possibility: &[Pitch],
    info: Option<&ChordInfo>,
    check: impl Fn(&Chord) -> Result<()>,
) -> Result<ChordInfo> {
    match info {
        Some(info) => Ok(info.clone()),
        None => {
            let chord = Chord::new(possibility)?;
            check(&chord)?;
            Ok(ChordInfo::of(&chord))
        }
    }
}

/// The notes of an augmented sixth, and which kind it is.
fn augmented_sixth_notes(
    possibility: &[Pitch],
    kind: Option<AugmentedSixth>,
    info: Option<&ChordInfo>,
) -> Result<(AugmentedSixth, ChordInfo)> {
    let mut kind = kind;
    let info = match info {
        Some(info) => info.clone(),
        None => {
            let chord = Chord::new(possibility)?;
            if kind.is_none() {
                kind = Some(AugmentedSixth::of(&chord)?);
            } else if !chord.is_augmented_sixth(false) {
                return Err(Error::FiguredBass(
                    "Possibility is not an augmented sixth chord.".to_string(),
                ));
            }
            ChordInfo::of(&chord)
        }
    };
    let kind = kind.ok_or_else(|| Error::FiguredBass("Unknown augSixthType: None".to_string()))?;
    Ok((kind, info))
}

/// The bass, root, fifth and fourth note of an augmented sixth, read as its
/// kind reads them: the fourth note stands where a French or Swiss sixth has
/// its root and where a German sixth has its seventh.
fn augmented_sixth_parts(kind: AugmentedSixth, info: &ChordInfo) -> [Option<&Pitch>; 4] {
    match kind {
        AugmentedSixth::French | AugmentedSixth::Swiss => [
            info.bass.as_ref(),
            info.third.as_ref(),
            info.seventh.as_ref(),
            info.root.as_ref(),
        ],
        AugmentedSixth::German => [
            info.bass.as_ref(),
            info.root.as_ref(),
            info.fifth.as_ref(),
            info.seventh.as_ref(),
        ],
    }
}

/// Resolves an augmented sixth to the dominant: music21's
/// `augmentedSixthToDominant`. The bass and the fifth fall a semitone and
/// the root rises one; a German sixth's seventh falls a semitone too, and a
/// Swiss sixth's doubly augmented fourth is respelled where it stands.
///
/// `kind` and `info` are read off the possibility where not given.
///
/// # Errors
///
/// A possibility that is no augmented sixth, or an Italian one, when they
/// are not given.
pub fn augmented_sixth_to_dominant(
    possibility: &[Pitch],
    kind: Option<AugmentedSixth>,
    info: Option<&ChordInfo>,
) -> Result<Vec<Pitch>> {
    resolve_augmented_sixth(
        possibility,
        kind,
        info,
        "-m2",
        &[
            (AugmentedSixth::Swiss, "d1"),
            (AugmentedSixth::German, "-m2"),
        ],
    )
}

/// Resolves an augmented sixth to the major tonic in second inversion:
/// music21's `augmentedSixthToMajorTonic`.
///
/// # Errors
///
/// As [`augmented_sixth_to_dominant`].
pub fn augmented_sixth_to_major_tonic(
    possibility: &[Pitch],
    kind: Option<AugmentedSixth>,
    info: Option<&ChordInfo>,
) -> Result<Vec<Pitch>> {
    resolve_augmented_sixth(
        possibility,
        kind,
        info,
        "P1",
        &[
            (AugmentedSixth::French, "M2"),
            (AugmentedSixth::German, "A1"),
            (AugmentedSixth::Swiss, "m2"),
        ],
    )
}

/// Resolves an augmented sixth to the minor tonic in second inversion:
/// music21's `augmentedSixthToMinorTonic`.
///
/// # Errors
///
/// As [`augmented_sixth_to_dominant`].
pub fn augmented_sixth_to_minor_tonic(
    possibility: &[Pitch],
    kind: Option<AugmentedSixth>,
    info: Option<&ChordInfo>,
) -> Result<Vec<Pitch>> {
    resolve_augmented_sixth(
        possibility,
        kind,
        info,
        "P1",
        &[
            (AugmentedSixth::French, "m2"),
            (AugmentedSixth::Swiss, "d2"),
        ],
    )
}

/// Resolves an augmented sixth: the bass falls and the root rises a minor
/// second, the fifth moves by `fifth`, and the last note moves as `other`
/// says for the kind of sixth it is, staying put for a kind not listed.
fn resolve_augmented_sixth(
    possibility: &[Pitch],
    kind: Option<AugmentedSixth>,
    info: Option<&ChordInfo>,
    fifth: &'static str,
    other: &[(AugmentedSixth, &'static str)],
) -> Result<Vec<Pitch>> {
    let (kind, info) = augmented_sixth_notes(possibility, kind, info)?;
    let [bass, root, fifth_note, other_note] = augmented_sixth_parts(kind, &info);
    let mut moves = vec![(bass, "-m2"), (root, "m2"), (fifth_note, fifth)];
    if let Some(&(_, step)) = other.iter().find(|(listed, _)| *listed == kind) {
        moves.push((other_note, step));
    }
    move_named(possibility, &moves)
}

fn dominant_seventh(chord: &Chord) -> Result<()> {
    if chord.is_dominant_seventh() {
        Ok(())
    } else {
        Err(Error::FiguredBass(
            "Possibility is not a dominant seventh chord.".to_string(),
        ))
    }
}

fn dominant_seventh_in_root_position(chord: &Chord) -> Result<()> {
    dominant_seventh(chord)?;
    if chord.inversion() == Some(0) {
        Ok(())
    } else {
        Err(Error::FiguredBass(
            "Possibility must be in root position.".to_string(),
        ))
    }
}

fn diminished_seventh(chord: &Chord) -> Result<()> {
    if chord.is_diminished_seventh() {
        Ok(())
    } else {
        Err(Error::FiguredBass(
            "Possibility is not a fully diminished seventh chord.".to_string(),
        ))
    }
}

/// The five notes of a seventh chord, each of which a rule may need.
fn seventh_notes(info: &ChordInfo) -> [Option<&Pitch>; 5] {
    [
        info.bass.as_ref(),
        info.root.as_ref(),
        info.third.as_ref(),
        info.fifth.as_ref(),
        info.seventh.as_ref(),
    ]
}

/// Resolves a dominant seventh to the major tonic: music21's
/// `dominantSeventhToMajorTonic`. The root in the bass rises a fourth, the
/// third rises a semitone, and the fifth and seventh fall -- or, taking a
/// V4/3 to I6, rise a tone.
///
/// # Errors
///
/// A possibility that is no dominant seventh, when `info` is not given.
pub fn dominant_seventh_to_major_tonic(
    possibility: &[Pitch],
    resolve_v43_to_i6: bool,
    info: Option<&ChordInfo>,
) -> Result<Vec<Pitch>> {
    dominant_seventh_to_tonic(possibility, resolve_v43_to_i6, info, "M2", "-M2", "-m2")
}

/// Resolves a dominant seventh to the minor tonic: music21's
/// `dominantSeventhToMinorTonic`.
///
/// # Errors
///
/// As [`dominant_seventh_to_major_tonic`].
pub fn dominant_seventh_to_minor_tonic(
    possibility: &[Pitch],
    resolve_v43_to_i6: bool,
    info: Option<&ChordInfo>,
) -> Result<Vec<Pitch>> {
    dominant_seventh_to_tonic(possibility, resolve_v43_to_i6, info, "m2", "-M2", "-M2")
}

fn dominant_seventh_to_tonic(
    possibility: &[Pitch],
    resolve_v43_to_i6: bool,
    info: Option<&ChordInfo>,
    fifth_up: &'static str,
    fifth_down: &'static str,
    seventh_down: &'static str,
) -> Result<Vec<Pitch>> {
    let info = info_or_read(possibility, info, dominant_seventh)?;
    let [bass, root, third, fifth, seventh] = seventh_notes(&info);
    let rules: Vec<Rule<'_>> = vec![
        (
            Box::new(move |pitch: &Pitch| named(root)(pitch) && bass == Some(pitch)),
            "P4",
        ),
        (Box::new(named(third)), "m2"),
        (
            Box::new(move |pitch: &Pitch| resolve_v43_to_i6 && named(fifth)(pitch)),
            fifth_up,
        ),
        (Box::new(named(fifth)), fifth_down),
        (
            Box::new(move |pitch: &Pitch| resolve_v43_to_i6 && named(seventh)(pitch)),
            "M2",
        ),
        (Box::new(named(seventh)), seventh_down),
    ];
    resolve(possibility, &rules)
}

/// Resolves a dominant seventh in root position deceptively, to the major
/// submediant: music21's `dominantSeventhToMajorSubmediant`.
///
/// # Errors
///
/// A possibility that is no dominant seventh in root position, when `info`
/// is not given.
pub fn dominant_seventh_to_major_submediant(
    possibility: &[Pitch],
    info: Option<&ChordInfo>,
) -> Result<Vec<Pitch>> {
    let info = info_or_read(possibility, info, dominant_seventh_in_root_position)?;
    let [_, root, third, fifth, seventh] = seventh_notes(&info);
    move_named(
        possibility,
        &[
            (root, "m2"),
            (third, "m2"),
            (fifth, "-M2"),
            (seventh, "-M2"),
        ],
    )
}

/// Resolves a dominant seventh in root position deceptively, to the minor
/// submediant: music21's `dominantSeventhToMinorSubmediant`.
///
/// # Errors
///
/// As [`dominant_seventh_to_major_submediant`].
pub fn dominant_seventh_to_minor_submediant(
    possibility: &[Pitch],
    info: Option<&ChordInfo>,
) -> Result<Vec<Pitch>> {
    let info = info_or_read(possibility, info, dominant_seventh_in_root_position)?;
    let [_, root, third, fifth, seventh] = seventh_notes(&info);
    move_named(
        possibility,
        &[
            (root, "M2"),
            (third, "m2"),
            (fifth, "-M2"),
            (seventh, "-m2"),
        ],
    )
}

/// Resolves a dominant seventh in root position to the major subdominant:
/// music21's `dominantSeventhToMajorSubdominant`.
///
/// # Errors
///
/// As [`dominant_seventh_to_major_submediant`].
pub fn dominant_seventh_to_major_subdominant(
    possibility: &[Pitch],
    info: Option<&ChordInfo>,
) -> Result<Vec<Pitch>> {
    let info = info_or_read(possibility, info, dominant_seventh_in_root_position)?;
    let [_, root, third, fifth, _] = seventh_notes(&info);
    move_named(possibility, &[(root, "M2"), (third, "m2"), (fifth, "-M2")])
}

/// Resolves a dominant seventh in root position to the minor subdominant:
/// music21's `dominantSeventhToMinorSubdominant`.
///
/// # Errors
///
/// As [`dominant_seventh_to_major_submediant`].
pub fn dominant_seventh_to_minor_subdominant(
    possibility: &[Pitch],
    info: Option<&ChordInfo>,
) -> Result<Vec<Pitch>> {
    let info = info_or_read(possibility, info, dominant_seventh_in_root_position)?;
    let [_, root, third, fifth, _] = seventh_notes(&info);
    move_named(possibility, &[(root, "m2"), (third, "m2"), (fifth, "-M2")])
}

/// Resolves a diminished seventh to the major tonic: music21's
/// `diminishedSeventhToMajorTonic`. With `doubled_root`, the third falls to
/// double the tonic rather than rising to its third.
///
/// # Errors
///
/// A possibility that is no fully diminished seventh, when `info` is not
/// given.
pub fn diminished_seventh_to_major_tonic(
    possibility: &[Pitch],
    doubled_root: bool,
    info: Option<&ChordInfo>,
) -> Result<Vec<Pitch>> {
    diminished_seventh_to_tonic(possibility, doubled_root, info, "M2", "-m2")
}

/// Resolves a diminished seventh to the minor tonic: music21's
/// `diminishedSeventhToMinorTonic`.
///
/// # Errors
///
/// As [`diminished_seventh_to_major_tonic`].
pub fn diminished_seventh_to_minor_tonic(
    possibility: &[Pitch],
    doubled_root: bool,
    info: Option<&ChordInfo>,
) -> Result<Vec<Pitch>> {
    diminished_seventh_to_tonic(possibility, doubled_root, info, "m2", "-M2")
}

fn diminished_seventh_to_tonic(
    possibility: &[Pitch],
    doubled_root: bool,
    info: Option<&ChordInfo>,
    third_up: &'static str,
    fifth_down: &'static str,
) -> Result<Vec<Pitch>> {
    let info = info_or_read(possibility, info, diminished_seventh)?;
    let [_, root, third, fifth, seventh] = seventh_notes(&info);
    let rules: Vec<Rule<'_>> = vec![
        (Box::new(named(root)), "m2"),
        (
            Box::new(move |pitch: &Pitch| doubled_root && named(third)(pitch)),
            "-M2",
        ),
        (Box::new(named(third)), third_up),
        (Box::new(named(fifth)), fifth_down),
        (Box::new(named(seventh)), "-m2"),
    ];
    resolve(possibility, &rules)
}

/// Resolves a diminished seventh to the major subdominant: music21's
/// `diminishedSeventhToMajorSubdominant`.
///
/// # Errors
///
/// As [`diminished_seventh_to_major_tonic`].
pub fn diminished_seventh_to_major_subdominant(
    possibility: &[Pitch],
    info: Option<&ChordInfo>,
) -> Result<Vec<Pitch>> {
    let info = info_or_read(possibility, info, diminished_seventh)?;
    let [_, root, third, _, seventh] = seventh_notes(&info);
    move_named(
        possibility,
        &[(root, "m2"), (third, "-M2"), (seventh, "A1")],
    )
}

/// Resolves a diminished seventh to the minor subdominant: music21's
/// `diminishedSeventhToMinorSubdominant`.
///
/// # Errors
///
/// As [`diminished_seventh_to_major_tonic`].
pub fn diminished_seventh_to_minor_subdominant(
    possibility: &[Pitch],
    info: Option<&ChordInfo>,
) -> Result<Vec<Pitch>> {
    let info = info_or_read(possibility, info, diminished_seventh)?;
    let [_, root, third, _, _] = seventh_notes(&info);
    move_named(possibility, &[(root, "m2"), (third, "-M2")])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// music21's own French, German and Swiss sixths over B-2.
    fn sixths() -> [Vec<Pitch>; 3] {
        let chord = |names: [&str; 4]| names.map(|name| Pitch::from_name(name).unwrap()).to_vec();
        [
            chord(["G#4", "E4", "D4", "B-2"]),
            chord(["G#4", "F4", "D4", "B-2"]),
            chord(["G#4", "E#4", "D4", "B-2"]),
        ]
    }

    fn names(pitches: Result<Vec<Pitch>>) -> Vec<String> {
        pitches
            .unwrap()
            .iter()
            .map(Pitch::name_with_octave)
            .collect()
    }

    #[test]
    fn every_augmented_sixth_resolves_as_music21_resolves_it() {
        for sixth in sixths() {
            assert_eq!(
                names(augmented_sixth_to_dominant(&sixth, None, None)),
                ["A4", "E4", "C#4", "A2"]
            );
            assert_eq!(
                names(augmented_sixth_to_major_tonic(&sixth, None, None)),
                ["A4", "F#4", "D4", "A2"]
            );
            assert_eq!(
                names(augmented_sixth_to_minor_tonic(&sixth, None, None)),
                ["A4", "F4", "D4", "A2"]
            );
        }
    }
}
