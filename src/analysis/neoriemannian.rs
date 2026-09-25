//! Neo-Riemannian transformations of triads: music21's
//! `analysis.neoRiemannian`.
//!
//! Each of L, P and R moves one pitch of a major or minor triad and keeps
//! the other two, turning major into minor and back:
//!
//! ```text
//!           C E G (C major)
//!   L: C -> B      B E G   (E minor)
//!   P: E -> E-     C E- G  (C minor)
//!   R: G -> A      C E A   (A minor)
//! ```
//!
//! Chains of them name the other relations: slide is `LPR`, the
//! Nebenverwandt `RLP`, the chromatic and disjunct mediants two or three
//! steps each, and `PLPLPL` walks a hexatonic cycle back to where it began.

use crate::{
    chord::Chord,
    error::{Error, Result},
    interval::Interval,
    pitch::Pitch,
};

use super::enharmonics::{EnharmonicRules, best_spelling};

/// The Forte class every major and minor triad belongs to.
const TRIAD_FORTE_CLASS: &str = "3-11";

/// The hexatonic cycle: `PLPLPL`.
const HEXATONIC_CYCLE: [Transform; 6] = [
    Transform::P,
    Transform::L,
    Transform::P,
    Transform::L,
    Transform::P,
    Transform::L,
];

/// Slide: `LPR`.
const SLIDE: [Transform; 3] = [Transform::L, Transform::P, Transform::R];

/// Nebenverwandt: `RLP`.
const NEBENVERWANDT: [Transform; 3] = [Transform::R, Transform::L, Transform::P];

/// One of the three neo-Riemannian transformations.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Transform {
    /// Leading-tone exchange: a major root falls a semitone, a minor fifth
    /// rises one. C major becomes E minor.
    L,
    /// Parallel: the third moves a semitone. C major becomes C minor.
    P,
    /// Relative: a major fifth rises a tone, a minor root falls one. C
    /// major becomes A minor.
    R,
}

/// The order a chain of transformations is read in.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ChainOrder {
    /// `LPR` is L, then P, then R. music21's default.
    #[default]
    LeftToRight,
    /// `LPR` is R, then P, then L: function notation, music21's
    /// `leftOrdered=True`.
    RightToLeft,
}

/// Whether a result keeps the spelling the transformations gave it.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Respell {
    /// Keep double flats and sharps where they arise: `Dbb4 Fb4 Abb4`.
    #[default]
    Keep,
    /// Respell with the best enharmonics: `C4 E4 G4`.
    Simplify,
}

/// A chromatic mediant: root motion by a third, mode kept, one common tone.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Mediant {
    /// Upper flat mediant, `UFM`: C major to E- major.
    UpperFlat,
    /// Upper sharp mediant, `USM`: C major to E major.
    UpperSharp,
    /// Lower flat mediant, `LFM`: C major to A- major.
    LowerFlat,
    /// Lower sharp mediant, `LSM`: C major to A major.
    LowerSharp,
}

/// Which of the two disjunct mediants: root motion by a chromatic third,
/// mode changed, no common tone.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum MediantSide {
    /// The mediant above: C major to E- minor.
    Upper,
    /// The mediant below: C major to G# minor.
    Lower,
}

/// Richard Cohn's four hexatonic systems, each the six triads one cycle of
/// `PL` passes through, named by the pitch classes of their roots.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum HexatonicSystem {
    /// Roots 0, 4 and 8: C, E, A-.
    Northern,
    /// Roots 1, 5 and 9: C#, F, A.
    Eastern,
    /// Roots 2, 6 and 10: D, F#, B-.
    Southern,
    /// Roots 3, 7 and 11: E-, G, B.
    Western,
}

impl Transform {
    /// The transformation a letter names: `L`, `P` or `R`.
    ///
    /// # Errors
    ///
    /// Any other letter.
    pub fn from_symbol(symbol: char) -> Result<Self> {
        match symbol {
            'L' => Ok(Self::L),
            'P' => Ok(Self::P),
            'R' => Ok(Self::R),
            other => Err(Error::Analysis(format!(
                "{other} is not a NeoRiemannian transformation (L, R, or P)"
            ))),
        }
    }

    /// The letter naming the transformation.
    pub fn symbol(self) -> char {
        match self {
            Self::L => 'L',
            Self::P => 'P',
            Self::R => 'R',
        }
    }

    /// The transformations a string of letters names, in the order written.
    ///
    /// # Errors
    ///
    /// A letter other than `L`, `P` or `R`.
    pub fn parse_chain(symbols: &str) -> Result<Vec<Self>> {
        symbols.chars().map(Self::from_symbol).collect()
    }

    /// music21's `L`, `P` and `R`: `chord` transformed, every other pitch
    /// kept where it was. The result's spelling is no longer inferred, and
    /// it keeps `chord`'s duration.
    ///
    /// ```
    /// use music21_rs::{Chord, Pitch, analysis::neoriemannian::Transform};
    ///
    /// let c_major = Chord::new("C4 E4 G4")?;
    /// let e_minor = Transform::L.apply(&c_major)?;
    /// assert_eq!(e_minor.pitches().iter().map(Pitch::name_with_octave).collect::<Vec<_>>(), ["B3", "E4", "G4"]);
    /// # Ok::<(), music21_rs::Error>(())
    /// ```
    ///
    /// # Errors
    ///
    /// A chord that is not a major or minor triad.
    pub fn apply(self, chord: &Chord) -> Result<Chord> {
        let (interval, changing) = if chord.is_major_triad() {
            match self {
                Self::L => ("-m2", chord.root()),
                Self::P => ("-A1", chord.third()),
                Self::R => ("M2", chord.fifth()),
            }
        } else if chord.is_minor_triad() {
            match self {
                Self::L => ("m2", chord.fifth()),
                Self::P => ("A1", chord.third()),
                Self::R => ("-M2", chord.root()),
            }
        } else {
            return Err(self.not_a_triad());
        };

        let changing = changing.ok_or_else(|| self.not_a_triad())?;
        single_pitch_transform(chord, &Interval::from_name(interval)?, changing)
    }

    /// music21's refusal, which spells the modes in capitals for P and R.
    fn not_a_triad(self) -> Error {
        let modes = match self {
            Self::L => "major or minor",
            Self::P | Self::R => "Major or Minor",
        };
        Error::Analysis(format!(
            "Cannot perform {} on this chord: not a {modes} triad",
            self.symbol()
        ))
    }
}

impl Mediant {
    /// Every mediant, in the order music21 tries them.
    pub const ALL: [Self; 4] = [
        Self::UpperFlat,
        Self::UpperSharp,
        Self::LowerFlat,
        Self::LowerSharp,
    ];

    /// The mediant music21 abbreviates so: `UFM`, `USM`, `LFM` or `LSM`.
    ///
    /// # Errors
    ///
    /// Any other abbreviation.
    pub fn from_code(code: &str) -> Result<Self> {
        Self::ALL
            .into_iter()
            .find(|mediant| mediant.code() == code)
            .ok_or_else(|| {
                Error::Value(
                    "Transformation must be one of ['UFM', 'USM', 'LFM', 'LSM']".to_string(),
                )
            })
    }

    /// music21's abbreviation.
    pub fn code(self) -> &'static str {
        match self {
            Self::UpperFlat => "UFM",
            Self::UpperSharp => "USM",
            Self::LowerFlat => "LFM",
            Self::LowerSharp => "LSM",
        }
    }

    /// The chain reaching the mediant from a major triad, read left to
    /// right; from a minor one it is read the other way.
    fn chain(self) -> [Transform; 2] {
        match self {
            Self::UpperFlat => [Transform::P, Transform::R],
            Self::UpperSharp => [Transform::L, Transform::P],
            Self::LowerFlat => [Transform::P, Transform::L],
            Self::LowerSharp => [Transform::R, Transform::P],
        }
    }
}

impl HexatonicSystem {
    /// music21's lowercase name: `northern`, `eastern`, `southern` or
    /// `western`.
    pub fn name(self) -> &'static str {
        match self {
            Self::Northern => "northern",
            Self::Eastern => "eastern",
            Self::Southern => "southern",
            Self::Western => "western",
        }
    }
}

/// Moves every pitch spelled as `changing` by `interval` and builds a new
/// chord of the lot: music21's `_singlePitchTransform`.
fn single_pitch_transform(chord: &Chord, interval: &Interval, changing: &Pitch) -> Result<Chord> {
    let target = changing.name();
    let mut pitches = chord.pitches();
    for pitch in &mut pitches {
        pitch.set_spelling_is_inferred(false);
        if pitch.name() != target {
            continue;
        }
        *pitch = pitch.transpose(interval)?;
    }

    with_duration_of(chord, Chord::new(pitches.as_slice())?)
}

/// `made` with `source`'s duration, as music21 copies `quarterLength` over.
fn with_duration_of(source: &Chord, mut made: Chord) -> Result<Chord> {
    if let Some(duration) = source.duration() {
        made.set_duration(duration.clone());
    }
    Ok(made)
}

/// `chord`'s pitch names with their octaves, as music21 prints a chord.
fn names_with_octave(chord: &Chord) -> Vec<String> {
    chord
        .pitches()
        .iter()
        .map(Pitch::name_with_octave)
        .collect()
}

/// The refusal of a chord a chain cannot start from.
fn not_a_triad_chain(chord: &Chord) -> Error {
    Error::Analysis(format!(
        "Cannot perform transformations on chord {}: not a major or minor triad",
        names_with_octave(chord).join(" ")
    ))
}

/// `chord` respelled with the best enharmonics, duration kept: music21's
/// `_simplerEnharmonics`. `B# F- G` comes back as `C E G`.
///
/// # Errors
///
/// An empty chord, or a pitch base 40 has no place for.
pub fn simpler_enharmonics(chord: &Chord) -> Result<Chord> {
    let best = best_spelling(&chord.pitches(), EnharmonicRules::MELODIC)?;
    with_duration_of(chord, Chord::new(best.as_slice())?)
}

/// Every chord a chain of transformations passes through, in order:
/// music21's `LRP_combinations` with `eachOne=True`. With
/// [`Respell::Simplify`] each is respelled on its own; the chain itself
/// runs on the spellings the transformations give.
///
/// # Errors
///
/// A chord that is not a major or minor triad.
pub fn lrp_chain(
    chord: &Chord,
    transforms: &[Transform],
    order: ChainOrder,
    respell: Respell,
) -> Result<Vec<Chord>> {
    if !chord.is_major_triad() && !chord.is_minor_triad() {
        return Err(not_a_triad_chain(chord));
    }

    let mut transforms = transforms.to_vec();
    if order == ChainOrder::RightToLeft {
        transforms.reverse();
    }

    let mut chain: Vec<Chord> = Vec::with_capacity(transforms.len());
    for transform in transforms {
        let last = chain.last().unwrap_or(chord);
        chain.push(transform.apply(last)?);
    }

    if respell == Respell::Keep {
        return Ok(chain);
    }
    chain.iter().map(simpler_enharmonics).collect()
}

/// Where a chain of transformations ends: music21's `LRP_combinations`.
/// No transformations at all leave `chord` as it was.
///
/// ```
/// use music21_rs::{Chord, Pitch, analysis::neoriemannian::*};
///
/// let c_major = Chord::new("C4 E4 G4")?;
/// let lp = Transform::parse_chain("LP")?;
/// let ended = lrp_combination(&c_major, &lp, ChainOrder::LeftToRight, Respell::Keep)?;
/// assert_eq!(ended.pitches().iter().map(Pitch::name_with_octave).collect::<Vec<_>>(), ["B3", "E4", "G#4"]);
/// # Ok::<(), music21_rs::Error>(())
/// ```
///
/// # Errors
///
/// A chord that is not a major or minor triad.
pub fn lrp_combination(
    chord: &Chord,
    transforms: &[Transform],
    order: ChainOrder,
    respell: Respell,
) -> Result<Chord> {
    let chain = lrp_chain(chord, transforms, order, Respell::Keep)?;
    let last = chain.last().unwrap_or(chord);

    match respell {
        Respell::Keep => Ok(last.clone()),
        Respell::Simplify => simpler_enharmonics(last),
    }
}

/// The six triads of `chord`'s hexatonic cycle, `PLPLPL`, ending on
/// `chord` again or its enharmonic: music21's `completeHexatonic`. With
/// [`Respell::Simplify`] each step is respelled before the next is taken.
///
/// # Errors
///
/// A chord that is not a major or minor triad.
pub fn complete_hexatonic(chord: &Chord, respell: Respell) -> Result<Vec<Chord>> {
    if chord.forte_class_tni().as_deref() != Some(TRIAD_FORTE_CLASS) {
        return Err(Error::Analysis(
            "Cannot perform transformations on this chord: not a major or minor triad".to_string(),
        ));
    }

    let mut cycle: Vec<Chord> = Vec::with_capacity(HEXATONIC_CYCLE.len());
    for transform in HEXATONIC_CYCLE {
        let last = cycle.last().unwrap_or(chord);
        let mut next = transform.apply(last)?;
        if respell == Respell::Simplify {
            next = simpler_enharmonics(&next)?;
        }
        cycle.push(next);
    }
    Ok(cycle)
}

/// The hexatonic system of `chord`'s root: music21's `hexatonicSystem`.
/// Only the root's pitch class counts, so sevenths and diminished triads
/// are classed too.
///
/// # Errors
///
/// A chord with no root.
pub fn hexatonic_system(chord: &Chord) -> Result<HexatonicSystem> {
    const SYSTEMS: [HexatonicSystem; 4] = [
        HexatonicSystem::Northern,
        HexatonicSystem::Eastern,
        HexatonicSystem::Southern,
        HexatonicSystem::Western,
    ];

    let root = chord
        .root()
        .ok_or_else(|| Error::Analysis("no pitches in chord".to_string()))?;

    // Roots a major third apart share a system, so the system is the pitch
    // class modulo four: C (0), E (4) and A- (8) are northern.
    let pitch_class = root.pitch_class().number() as usize;
    Ok(SYSTEMS[pitch_class % SYSTEMS.len()])
}

/// `chord`'s chromatic mediant, respelled: music21's `chromaticMediants`.
///
/// # Errors
///
/// A chord that is not a major or minor triad.
pub fn chromatic_mediant(chord: &Chord, mediant: Mediant) -> Result<Chord> {
    let order = if chord.is_minor_triad() {
        ChainOrder::RightToLeft
    } else if chord.is_major_triad() {
        ChainOrder::LeftToRight
    } else {
        return Err(Error::Value("Chord must be major or minor".to_string()));
    };

    lrp_combination(chord, &mediant.chain(), order, Respell::Simplify)
}

/// `chord`'s disjunct mediant on `side`, respelled: music21's
/// `disjunctMediants`.
///
/// # Errors
///
/// A chord that is not a major or minor triad.
pub fn disjunct_mediant(chord: &Chord, side: MediantSide) -> Result<Chord> {
    const PRP: [Transform; 3] = [Transform::P, Transform::R, Transform::P];
    const PLP: [Transform; 3] = [Transform::P, Transform::L, Transform::P];

    // Upper from major and lower from minor is PRP; the other two PLP.
    let chain = match (chord.is_major_triad(), chord.is_minor_triad(), side) {
        (true, _, MediantSide::Upper) | (_, true, MediantSide::Lower) => PRP,
        (true, _, MediantSide::Lower) | (_, true, MediantSide::Upper) => PLP,
        _ => return Err(Error::Value("Chord must be major or minor".to_string())),
    };

    lrp_combination(chord, &chain, ChainOrder::LeftToRight, Respell::Simplify)
}

/// Slide, `LPR`: the third held, major root a semitone below the minor.
/// music21's `S`.
///
/// # Errors
///
/// A chord that is not a major or minor triad.
pub fn slide(chord: &Chord) -> Result<Chord> {
    lrp_combination(chord, &SLIDE, ChainOrder::LeftToRight, Respell::Keep)
}

/// Nebenverwandt, `RLP`: a minor triad to its major dominant, a major one
/// to its minor subdominant. music21's `N`.
///
/// # Errors
///
/// A chord that is not a major or minor triad.
pub fn nebenverwandt(chord: &Chord) -> Result<Chord> {
    lrp_combination(
        chord,
        &NEBENVERWANDT,
        ChainOrder::LeftToRight,
        Respell::Keep,
    )
}

/// The first of `transforms` taking `from` to `to`'s normal order:
/// music21's `isNeoR`.
///
/// # Errors
///
/// `from` is not a major or minor triad.
pub fn is_neo_r(from: &Chord, to: &Chord, transforms: &[Transform]) -> Result<Option<Transform>> {
    let target = to.normal_order();
    for transform in transforms {
        if transform.apply(from)?.normal_order() == target {
            return Ok(Some(*transform));
        }
    }
    Ok(None)
}

/// The chromatic mediant taking `from` to `to`'s normal order: music21's
/// `isChromaticMediant`.
///
/// # Errors
///
/// `from` is not a major or minor triad.
pub fn is_chromatic_mediant(from: &Chord, to: &Chord) -> Result<Option<Mediant>> {
    let target = to.normal_order();
    for mediant in Mediant::ALL {
        if chromatic_mediant(from, mediant)?.normal_order() == target {
            return Ok(Some(mediant));
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::duration::Duration;

    fn chord(names: &str) -> Chord {
        Chord::new(names).unwrap()
    }

    fn named(chord: &Chord) -> Vec<String> {
        names_with_octave(chord)
    }

    fn all_named(chords: &[Chord]) -> Vec<Vec<String>> {
        chords.iter().map(named).collect()
    }

    fn chain(symbols: &str) -> Vec<Transform> {
        Transform::parse_chain(symbols).unwrap()
    }

    #[test]
    fn each_transform_moves_one_pitch() {
        let c_major = chord("C4 E4 G4");
        assert_eq!(
            named(&Transform::L.apply(&c_major).unwrap()),
            ["B3", "E4", "G4"]
        );
        assert_eq!(
            named(&Transform::P.apply(&c_major).unwrap()),
            ["C4", "Eb4", "G4"]
        );
        assert_eq!(
            named(&Transform::R.apply(&c_major).unwrap()),
            ["C4", "E4", "A4"]
        );
    }

    #[test]
    fn integer_chords_have_their_spelling_fixed() {
        let c_minor = Chord::new([0, 3, 7].as_slice()).unwrap();
        let l = Transform::L.apply(&c_minor).unwrap();
        assert_eq!(named(&l), ["C", "Eb", "Ab"]);
        assert!(l.pitches().iter().all(|p| !p.spelling_is_inferred()));
    }

    #[test]
    fn a_non_triad_is_refused() {
        let error = Transform::L.apply(&chord("C4 D4 E4")).unwrap_err();
        assert!(error.to_string().contains("Cannot perform L on this chord"));
        assert!(Transform::from_symbol('X').is_err());
    }

    #[test]
    fn chains_run_in_either_order() {
        let c_major = chord("C4 E4 G4");
        let lp = chain("LP");
        let forward = lrp_combination(&c_major, &lp, ChainOrder::LeftToRight, Respell::Keep);
        let backward = lrp_combination(&c_major, &lp, ChainOrder::RightToLeft, Respell::Keep);
        assert_eq!(named(&forward.unwrap()), ["B3", "E4", "G#4"]);
        assert_eq!(named(&backward.unwrap()), ["C4", "Eb4", "Ab4"]);

        let doubled = chord("C4 E4 G4 C5 E5");
        let rlp = lrp_combination(
            &doubled,
            &chain("RLP"),
            ChainOrder::LeftToRight,
            Respell::Keep,
        );
        assert_eq!(named(&rlp.unwrap()), ["C4", "F4", "Ab4", "C5", "F5"]);
    }

    #[test]
    fn a_hexatonic_chain_comes_back_respelled() {
        let hexatonic = chain("LPLPLP");
        let b_major = chord("B4 D#5 F#5");
        let back = lrp_combination(
            &b_major,
            &hexatonic,
            ChainOrder::RightToLeft,
            Respell::Simplify,
        );
        assert_eq!(named(&back.unwrap()), ["B4", "D#5", "F#5"]);

        let c_major = chord("C4 E4 G4");
        let kept = lrp_combination(&c_major, &hexatonic, ChainOrder::RightToLeft, Respell::Keep);
        assert_eq!(named(&kept.unwrap()), ["Dbb4", "Fb4", "Abb4"]);

        let a_flat = chord("A-4 C4 E-5");
        let sharp = lrp_combination(&a_flat, &hexatonic, ChainOrder::LeftToRight, Respell::Keep);
        assert_eq!(named(&sharp.unwrap()), ["G#4", "B#3", "D#5"]);
    }

    #[test]
    fn every_step_of_a_chain_can_be_kept() {
        let steps = lrp_chain(
            &chord("C4 E4 G4"),
            &chain("LPLPLP"),
            ChainOrder::LeftToRight,
            Respell::Simplify,
        );
        assert_eq!(
            all_named(&steps.unwrap()),
            [
                ["B3", "E4", "G4"],
                ["B3", "E4", "G#4"],
                ["B3", "D#4", "G#4"],
                ["C4", "Eb4", "Ab4"],
                ["C4", "Eb4", "G4"],
                ["C4", "E4", "G4"],
            ]
        );
    }

    #[test]
    fn the_hexatonic_cycle_is_music21s() {
        let c_major = chord("C4 E4 G4");
        assert_eq!(
            all_named(&complete_hexatonic(&c_major, Respell::Keep).unwrap()),
            [
                ["C4", "Eb4", "G4"],
                ["C4", "Eb4", "Ab4"],
                ["Cb4", "Eb4", "Ab4"],
                ["Cb4", "Fb4", "Ab4"],
                ["Cb4", "Fb4", "Abb4"],
                ["Dbb4", "Fb4", "Abb4"],
            ]
        );
        assert_eq!(
            all_named(&complete_hexatonic(&c_major, Respell::Simplify).unwrap()),
            [
                ["C4", "Eb4", "G4"],
                ["C4", "Eb4", "Ab4"],
                ["B3", "D#4", "G#4"],
                ["B3", "E4", "G#4"],
                ["B3", "E4", "G4"],
                ["C4", "E4", "G4"],
            ]
        );
        assert!(complete_hexatonic(&chord("C4 D4 E4"), Respell::Keep).is_err());
    }

    #[test]
    fn hexatonic_systems_follow_the_root() {
        assert_eq!(
            hexatonic_system(&chord("C E G")).unwrap(),
            HexatonicSystem::Northern
        );
        assert_eq!(
            hexatonic_system(&chord("G B- D")).unwrap(),
            HexatonicSystem::Western
        );
        assert_eq!(
            hexatonic_system(&chord("F#4 D5 A5 C6")).unwrap(),
            HexatonicSystem::Southern
        );
        assert!(hexatonic_system(&Chord::empty()).is_err());
    }

    #[test]
    fn mediants_are_music21s() {
        let c_major = chord("C5 E5 G5");
        let normal = |mediant| chromatic_mediant(&c_major, mediant).unwrap().normal_order();
        assert_eq!(normal(Mediant::UpperFlat), [3, 7, 10]);
        assert_eq!(normal(Mediant::UpperSharp), [4, 8, 11]);
        assert_eq!(
            named(&chromatic_mediant(&c_major, Mediant::LowerFlat).unwrap()),
            ["C5", "Eb5", "Ab5"]
        );
        assert_eq!(
            named(&chromatic_mediant(&c_major, Mediant::LowerSharp).unwrap()),
            ["C#5", "E5", "A5"]
        );
        assert_eq!(
            named(&chromatic_mediant(&c_major, Mediant::UpperSharp).unwrap()),
            ["B4", "E5", "G#5"]
        );

        let names = |c: Chord| -> Vec<String> { c.pitches().iter().map(Pitch::name).collect() };
        assert_eq!(
            names(disjunct_mediant(&c_major, MediantSide::Upper).unwrap()),
            ["Bb", "Eb", "Gb"]
        );
        assert_eq!(
            names(disjunct_mediant(&c_major, MediantSide::Lower).unwrap()),
            ["B", "D#", "G#"]
        );
        assert_eq!(names(slide(&c_major).unwrap()), ["C#", "E", "G#"]);
        assert_eq!(names(slide(&chord("A4 C5 E5")).unwrap()), ["Ab", "C", "Eb"]);
        assert_eq!(names(nebenverwandt(&c_major).unwrap()), ["C", "F", "Ab"]);
        assert_eq!(
            names(nebenverwandt(&chord("A4 C5 E5")).unwrap()),
            ["G#", "B", "E"]
        );

        assert_eq!(Mediant::from_code("LFM").unwrap(), Mediant::LowerFlat);
        assert!(Mediant::from_code("XYZ").is_err());
    }

    #[test]
    fn relations_are_recognised() {
        let c_major = chord("C4 E4 G4");
        let lrp = chain("LRP");
        assert_eq!(
            is_neo_r(&c_major, &chord("B3 E4 G4"), &lrp).unwrap(),
            Some(Transform::L)
        );
        assert_eq!(
            is_neo_r(&c_major, &chord("C4 E-4 G4"), &lrp).unwrap(),
            Some(Transform::P)
        );
        assert_eq!(
            is_neo_r(&c_major, &chord("C4 E4 A4"), &lrp).unwrap(),
            Some(Transform::R)
        );
        assert_eq!(
            is_neo_r(&c_major, &chord("C-4 E-4 A-4"), &lrp).unwrap(),
            None
        );
        assert_eq!(
            is_neo_r(&c_major, &chord("C4 E-4 G4"), &chain("LR")).unwrap(),
            None
        );

        assert_eq!(
            is_chromatic_mediant(&c_major, &chord("A-3 C4 E-4")).unwrap(),
            Some(Mediant::LowerFlat)
        );
        assert_eq!(
            is_chromatic_mediant(&c_major, &chord("C-4 E-4 A-4")).unwrap(),
            None
        );
    }

    #[test]
    fn minor_triads_take_their_mediants_as_music21_does() {
        let a_minor = chord("A4 C5 E5");
        let mediant = |mediant| named(&chromatic_mediant(&a_minor, mediant).unwrap());
        assert_eq!(mediant(Mediant::UpperFlat), ["G4", "C5", "Eb5"]);
        assert_eq!(mediant(Mediant::UpperSharp), ["G#4", "C#5", "E5"]);
        assert_eq!(mediant(Mediant::LowerFlat), ["Ab4", "C5", "F5"]);
        assert_eq!(mediant(Mediant::LowerSharp), ["A4", "C#5", "F#5"]);

        let side = |side| named(&disjunct_mediant(&a_minor, side).unwrap());
        assert_eq!(side(MediantSide::Upper), ["Ab4", "Db5", "F5"]);
        assert_eq!(side(MediantSide::Lower), ["A#4", "C#5", "F#5"]);
    }

    #[test]
    fn only_triads_take_mediants_or_chains() {
        let cluster = chord("C4 D4 E4");
        assert!(matches!(
            chromatic_mediant(&cluster, Mediant::UpperFlat),
            Err(Error::Value(_))
        ));
        assert!(matches!(
            disjunct_mediant(&cluster, MediantSide::Upper),
            Err(Error::Value(_))
        ));

        let chain = lrp_chain(
            &cluster,
            &[Transform::L],
            ChainOrder::LeftToRight,
            Respell::Keep,
        );
        let message = chain.unwrap_err().to_string();
        assert!(message.contains("Cannot perform transformations on chord C4 D4 E4"));
    }

    #[test]
    fn p_and_r_refuse_in_music21s_capitals() {
        let cluster = chord("C4 D4 E4");
        for transform in [Transform::P, Transform::R] {
            let message = transform.apply(&cluster).unwrap_err().to_string();
            let expected = format!(
                "Cannot perform {} on this chord: not a Major or Minor triad",
                transform.symbol()
            );
            assert!(message.contains(&expected), "{message}");
        }
    }

    #[test]
    fn a_chain_parses_back_to_its_letters() {
        let symbols: String = chain("LPR").into_iter().map(Transform::symbol).collect();
        assert_eq!(symbols, "LPR");
    }

    #[test]
    fn a_result_lasts_as_long_as_its_source() {
        let mut c_major = chord("C4 E4 G4");
        c_major.set_duration(Duration::half());
        let lasts = |chord: &Chord| chord.duration().map(Duration::quarter_length);

        assert_eq!(lasts(&Transform::L.apply(&c_major).unwrap()), Some(2.0));
        let cycle = complete_hexatonic(&c_major, Respell::Simplify).unwrap();
        assert_eq!(lasts(&cycle[5]), Some(2.0));
    }

    #[test]
    fn hexatonic_systems_have_music21s_names() {
        let names: Vec<&str> = [
            HexatonicSystem::Northern,
            HexatonicSystem::Eastern,
            HexatonicSystem::Southern,
            HexatonicSystem::Western,
        ]
        .into_iter()
        .map(HexatonicSystem::name)
        .collect();
        assert_eq!(names, ["northern", "eastern", "southern", "western"]);
    }

    #[test]
    fn simpler_enharmonics_respell() {
        assert_eq!(
            named(&simpler_enharmonics(&chord("B# F- G")).unwrap()),
            ["C", "E", "G"]
        );
    }
}
