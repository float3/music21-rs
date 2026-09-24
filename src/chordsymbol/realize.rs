//! Sounding a chord symbol the way music21 sounds one: its
//! `ChordSymbol._updatePitches`, octaves and all.
//!
//! The order of the steps is music21's and it matters. The kind's notation
//! is laid out in the octave above the root, taken from octave three; the
//! upper notes of a ninth, eleventh or thirteenth are lifted an octave; a
//! bass the chord does not invert onto is added below it; the chord-step
//! modifications are applied to that list as it then stands; the notes below
//! an inverted bass go up; and the whole is moved into the middle of a piano.
//! Every one of those steps reads the list the one before it left, so a
//! subtraction pairs notes with degrees in whatever order the notes are in
//! by then — which is music21's, and is kept.

use crate::{
    defaults::IntegerType,
    error::{Error, Result},
    interval::Interval,
    pitch::Pitch,
    scale::{Scale, ScaleType},
};

use super::tables::notation_intervals;

/// What a [`ChordStepModification`] does to the chord: music21's `modType`.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ChordStepModificationType {
    /// A degree sounded beside those the kind has, or in place of one of
    /// them with the same number.
    Add,
    /// A degree the kind has, taken out.
    Subtract,
    /// A degree the kind has, raised or lowered.
    Alter,
}

impl ChordStepModificationType {
    /// music21's name for it: `add`, `subtract` or `alter`.
    #[must_use]
    pub fn music21_name(self) -> &'static str {
        match self {
            Self::Add => "add",
            Self::Subtract => "subtract",
            Self::Alter => "alter",
        }
    }

    /// Reads music21's name, in any case.
    pub fn from_music21_name(name: &str) -> Result<Self> {
        match name.to_ascii_lowercase().as_str() {
            "add" => Ok(Self::Add),
            "subtract" => Ok(Self::Subtract),
            "alter" => Ok(Self::Alter),
            _ => Err(Error::Chord(format!(
                "not a valid degree modification type: {name}"
            ))),
        }
    }
}

/// One degree added to, taken from or altered in a chord symbol: music21's
/// `ChordStepModification`, and MusicXML's `<degree>`.
///
/// The interval is how far the degree is moved, usually an augmented unison
/// one way or the other. An alteration given as a number of semitones is
/// spelled as music21 spells it — up to three as that many augmentations of
/// the unison, beyond that as the interval that many semitones make.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ChordStepModification {
    modification_type: ChordStepModificationType,
    degree: u8,
    interval: Interval,
}

impl ChordStepModification {
    /// A modification of a degree by a number of semitones.
    pub fn new(
        modification_type: ChordStepModificationType,
        degree: u8,
        alter: IntegerType,
    ) -> Result<Self> {
        Ok(Self {
            modification_type,
            degree,
            interval: interval_of_alteration(alter)?,
        })
    }

    /// A modification of a degree by an interval already in hand.
    pub fn with_interval(
        modification_type: ChordStepModificationType,
        degree: u8,
        interval: Interval,
    ) -> Self {
        Self {
            modification_type,
            degree,
            interval,
        }
    }

    /// What the modification does.
    #[must_use]
    pub fn modification_type(&self) -> ChordStepModificationType {
        self.modification_type
    }

    /// The degree it does it to, counted from the root as one.
    #[must_use]
    pub fn degree(&self) -> u8 {
        self.degree
    }

    /// How far the degree is moved.
    pub fn interval(&self) -> &Interval {
        &self.interval
    }
}

/// music21's reading of an alteration in semitones as an interval: a
/// perfect unison for none, augmented unisons up to three, and the interval
/// the semitones make past that.
fn interval_of_alteration(alter: IntegerType) -> Result<Interval> {
    let size = alter.unsigned_abs();
    if size > 3 {
        return Interval::from_semitones(alter);
    }
    let mut name = if size == 0 {
        "P".to_string()
    } else {
        "a".repeat(size as usize)
    };
    name.push('1');
    if alter < 0 {
        name.push('-');
    }
    Interval::from_name(&name)
}

/// A chord symbol sounded: its pitches with their octaves, lowest first, and
/// the root and bass among them.
#[derive(Clone, Debug)]
pub(super) struct Realized {
    pub(super) pitches: Vec<Pitch>,
    root: Pitch,
    bass: Pitch,
}

/// A chord of one of music21's kinds as music21 voices it: its pitches, and
/// the root and bass music21 holds as objects of their own. Those move with
/// the chord where it holds them and not where it does not, so they need
/// not be the first note of their name, nor a note of the chord at all:
/// `Ab10/F#` sounds `F#2 A-3 C#4 E4` over a root of A3.
#[derive(Clone, Debug, PartialEq)]
pub struct ChordVoicing {
    pitches: Vec<Pitch>,
    root: Pitch,
    bass: Pitch,
}

impl ChordVoicing {
    /// The pitches, lowest written first.
    pub fn pitches(&self) -> &[Pitch] {
        &self.pitches
    }

    /// The root, where music21's own root object ends up.
    pub fn root(&self) -> &Pitch {
        &self.root
    }

    /// The bass, where music21's own bass object ends up.
    pub fn bass(&self) -> &Pitch {
        &self.bass
    }
}

/// The inversion a bass stands the chord in, read off the interval from the
/// bass up to the root as music21's `Chord._findInversion` reads it: a sixth
/// is the first inversion, a fourth the second, a second the third, and a
/// seventh, fifth and third the fourth to the sixth. Anything else is no
/// inversion at all.
pub(super) fn inversion_between(root: &Pitch, bass: &Pitch) -> Result<Option<u8>> {
    // The size of the step up from the bass to the root, whichever octave
    // either was written in.
    let steps = (root.diatonic_note_number() - bass.diatonic_note_number()).rem_euclid(7) + 1;
    Ok(match steps {
        1 => Some(0),
        6 => Some(1),
        4 => Some(2),
        2 => Some(3),
        7 => Some(4),
        5 => Some(5),
        3 => Some(6),
        _ => None,
    })
}

/// Whether a chord of this kind can stand in this inversion: music21's
/// `ChordSymbol.inversionIsValid`, which reads the kind and not the notes —
/// so a dominant seventh with a ninth added still inverts as a seventh.
pub fn inversion_is_valid_for_kind(kind: &str, inversion: u8) -> bool {
    const SEVENTHS: [&str; 12] = [
        "French",
        "German",
        "Italian",
        "Neapolitan",
        "Tristan",
        "augmented-seventh",
        "diminished-seventh",
        "dominant-seventh",
        "half-diminished",
        "major-minor",
        "major-seventh",
        "minor-seventh",
    ];
    const NINTHS: [&str; 3] = ["dominant-ninth", "major-ninth", "minor-ninth"];
    const ELEVENTHS: [&str; 3] = ["dominant-11th", "major-11th", "minor-11th"];
    const THIRTEENTHS: [&str; 3] = ["dominant-13th", "major-13th", "minor-13th"];
    let within = |list: &[&str]| list.contains(&kind);
    match inversion {
        5 => within(&THIRTEENTHS) || within(&ELEVENTHS),
        4 => within(&ELEVENTHS) || within(&THIRTEENTHS) || within(&NINTHS),
        3 => within(&SEVENTHS) || within(&NINTHS) || within(&ELEVENTHS) || within(&THIRTEENTHS),
        1 | 2 => kind != "pedal",
        _ => false,
    }
}

/// The pitches a chord of one of music21's kinds sounds on a root, with a
/// bass and chord-step modifications: music21's `ChordSymbol._updatePitches`
/// run over the state MusicXML hands it, rather than over a figure.
///
/// A kind music21's table has not got — or none at all — sounds its root,
/// and its bass beside it, as music21's does.
pub fn sound_chord_kind(
    root: &Pitch,
    kind: &str,
    bass: Option<&Pitch>,
    modifications: &[ChordStepModification],
) -> Result<Vec<Pitch>> {
    // The alias is read only to find the notation: music21 compares the
    // kind as written everywhere else, which is why its sevenths include
    // `half-diminished` and `major-minor`.
    let notation = super::tables::notation_for_kind(super::tables::resolve_kind_alias(kind));
    sound_chord_notation(root, kind, notation, bass, modifications)
}

/// The same with the kind's notation handed over rather than looked up:
/// music21's table can be added to while a program runs
/// (`addNewChordSymbol`), and a kind added there is sounded from the
/// notation it was added with.
pub fn sound_chord_notation(
    root: &Pitch,
    kind: &str,
    notation: Option<&str>,
    bass: Option<&Pitch>,
    modifications: &[ChordStepModification],
) -> Result<Vec<Pitch>> {
    Ok(realize(root, bass, Some(kind), notation, None, modifications)?.pitches)
}

/// The same, with the root and bass as music21 is left holding them: its
/// `_updatePitches` in full.
pub fn voice_chord_notation(
    root: &Pitch,
    kind: &str,
    notation: Option<&str>,
    bass: Option<&Pitch>,
    modifications: &[ChordStepModification],
) -> Result<ChordVoicing> {
    let realized = realize(root, bass, Some(kind), notation, None, modifications)?;
    Ok(ChordVoicing {
        pitches: realized.pitches,
        root: realized.root,
        bass: realized.bass,
    })
}

/// A pitch named `name` in `octave`.
fn named(name: &str, octave: IntegerType) -> Result<Pitch> {
    let mut pitch = Pitch::from_name(name)?;
    pitch.set_octave(Some(octave));
    Ok(pitch)
}

fn octave_of(pitch: &Pitch) -> IntegerType {
    pitch
        .octave()
        .unwrap_or(crate::defaults::PITCH_OCTAVE as IntegerType)
}

fn shift_octave(pitch: &mut Pitch, by: IntegerType) {
    let octave = octave_of(pitch);
    pitch.set_octave(Some(octave + by));
}

/// music21's `sortDiatonicAscending`: by staff position, then by pitch
/// space, so `B#3` comes before `C4`.
fn sort_diatonic(pitches: &mut [Pitch]) {
    pitches.sort_by(|left, right| {
        left.diatonic_note_number()
            .cmp(&right.diatonic_note_number())
            .then(left.ps().total_cmp(&right.ps()))
    });
}

/// music21's `FiguredBassScale.getSamplePitches` over the kind's notation,
/// with the duplicated root taken off the front: every note the notation
/// names, placed from the root upward to a diminished octave above it.
///
/// The names are the bass's and then the notation's own, last first, which
/// is the order music21 builds them in; the sort that follows is stable, so
/// notes that sound alike keep it.
fn sample_pitches(root: &Pitch, names: &[String]) -> Result<Vec<Pitch>> {
    let top = root.transpose(&Interval::from_name("d8")?)?;
    let highest_octave = octave_of(&top);
    let mut order = vec![root.name()];
    order.extend(names.iter().rev().cloned());
    let mut placed = Vec::new();
    for name in &order {
        for octave in 0..=highest_octave {
            let candidate = named(name, octave)?;
            if candidate.ps() >= root.ps() && candidate.ps() <= top.ps() {
                placed.push(candidate);
            }
        }
    }
    placed.sort_by(|left, right| left.ps().total_cmp(&right.ps()));
    if !placed.is_empty() {
        let _ = placed.remove(0);
    }
    Ok(placed)
}

/// Sounds a chord of a music21 kind on a root, with a bass and the
/// modifications read off its figure or handed over with it.
///
/// `names` are the notation's pitch names over the root and `degrees` the
/// notation's own tokens (`1`, `-3`, `5`), which the modifications are
/// matched against. `kind` decides the octave lifts and which inversions
/// the bass may make.
/// What one place in music21's list of pitches holds. music21 puts its
/// root and bass objects themselves in the list where it puts them there
/// at all, so an octave pass over the list moves them with it, and once for
/// every place they hold: a bass added twice is one note moved twice.
#[derive(Clone, Copy, PartialEq)]
enum Slot {
    Root,
    Bass,
    Own(usize),
}

/// music21's list of pitches with the identity of each place kept.
///
/// ```text
///   slots:  [Root, Bass, Own(0), Bass]
///   root    C3 <- one object, read and moved through its slot
///   bass    E2 <- one object, held twice, moved twice a pass
///   own     [G3]
/// ```
struct Voicing {
    root: Pitch,
    bass: Pitch,
    own: Vec<Pitch>,
    slots: Vec<Slot>,
}

impl Voicing {
    /// A list of pitches of their own.
    fn owning(root: Pitch, bass: Pitch, pitches: Vec<Pitch>) -> Self {
        let slots = (0..pitches.len()).map(Slot::Own).collect();
        Self {
            root,
            bass,
            own: pitches,
            slots,
        }
    }

    fn pitch(&self, slot: Slot) -> &Pitch {
        match slot {
            Slot::Root => &self.root,
            Slot::Bass => &self.bass,
            Slot::Own(index) => &self.own[index],
        }
    }

    fn pitch_mut(&mut self, slot: Slot) -> &mut Pitch {
        match slot {
            Slot::Root => &mut self.root,
            Slot::Bass => &mut self.bass,
            Slot::Own(index) => &mut self.own[index],
        }
    }

    /// The pitches the places hold, in order.
    fn values(&self) -> Vec<Pitch> {
        self.slots
            .iter()
            .map(|slot| self.pitch(*slot).clone())
            .collect()
    }

    /// Moves the pitch in the place at `index` by `by` octaves.
    fn shift(&mut self, index: usize, by: IntegerType) {
        let slot = self.slots[index];
        shift_octave(self.pitch_mut(slot), by);
    }

    /// Moves every place by `by` octaves, as music21's loop over the list
    /// does: a pitch held twice moves twice.
    fn shift_all(&mut self, by: IntegerType) {
        for index in 0..self.slots.len() {
            self.shift(index, by);
        }
    }

    /// Takes `pitches` as the list after a step that worked on its values:
    /// music21's modifications take pitches out and add new ones at the
    /// end, so the places that remain keep what they held, in order, and
    /// what follows them is new.
    fn replace(&mut self, pitches: Vec<Pitch>) {
        let old = std::mem::take(&mut self.slots);
        let mut remaining = old.into_iter().peekable();
        for pitch in pitches {
            // Skip the places taken out, up to one that holds this pitch.
            while remaining
                .peek()
                .is_some_and(|slot| *self.pitch(*slot) != pitch)
            {
                remaining.next();
            }
            match remaining.next() {
                Some(slot) => self.slots.push(slot),
                None => {
                    self.slots.push(Slot::Own(self.own.len()));
                    self.own.push(pitch);
                }
            }
        }
    }
}

pub(super) fn realize(
    root: &Pitch,
    bass: Option<&Pitch>,
    kind: Option<&str>,
    notation: Option<&str>,
    names: Option<Vec<String>>,
    modifications: &[ChordStepModification],
) -> Result<Realized> {
    let root = named(&root.name(), 3)?;
    let bass = named(&bass.unwrap_or(&root).name(), 3)?;
    let kind = kind.unwrap_or("");

    let mut degrees: Vec<String> = notation
        .map(|notation| notation.split(',').map(str::to_string).collect())
        .unwrap_or_default();
    let mut voicing = match (&names, notation) {
        (Some(names), _) => {
            Voicing::owning(root.clone(), bass.clone(), sample_pitches(&root, names)?)
        }
        (None, Some(notation)) => {
            let names = notation_intervals(notation)?
                .into_iter()
                .map(|(_, name)| Ok(Interval::from_name(&name)?.transpose_pitch(&root)?.name()))
                .collect::<Result<Vec<_>>>()?;
            Voicing::owning(root.clone(), bass.clone(), sample_pitches(&root, &names)?)
        }
        // A kind music21 has no notation for sounds its root, and its bass
        // beside it: the objects themselves.
        (None, None) => {
            let mut slots = vec![Slot::Root];
            if bass != root {
                slots.push(Slot::Bass);
            }
            Voicing {
                root: root.clone(),
                bass: bass.clone(),
                own: Vec::new(),
                slots,
            }
        }
    };

    // music21's `_adjustOctaves`: the upper notes of the stacked kinds go up
    // an octave, which is what spaces a ninth as a ninth. It builds new
    // pitches as it does.
    let lifted: &[usize] = match kind {
        "dominant-ninth" | "major-ninth" | "minor-ninth" => &[1],
        "dominant-11th" | "major-11th" | "minor-11th" => &[1, 3],
        "dominant-13th" | "major-13th" | "minor-13th" => &[1, 3, 5],
        _ => &[],
    };
    if !lifted.is_empty() {
        let mut pitches = voicing.values();
        for &index in lifted {
            if let Some(pitch) = pitches.get_mut(index) {
                shift_octave(pitch, 1);
            }
        }
        sort_diatonic(&mut pitches);
        voicing = Voicing::owning(voicing.root, voicing.bass, pitches);
    }

    let mut inversion = None;
    if voicing.root.name() != voicing.bass.name() {
        let found = inversion_between(&voicing.root, &voicing.bass)?;
        match found {
            Some(number) if inversion_is_valid_for_kind(kind, number) => inversion = Some(number),
            // A bass the chord does not invert onto is a note added under
            // it, in the octave below the root; the object moves, wherever
            // the list already holds it.
            _ => {
                voicing.bass.set_octave(Some(2));
                voicing.slots.push(Slot::Bass);
            }
        }
    }

    let mut pitches = voicing.values();
    let current_root = voicing.root.clone();
    apply_modifications(
        &current_root,
        kind,
        &mut pitches,
        &mut degrees,
        modifications,
    )?;
    voicing.replace(pitches);

    if let Some(number) = inversion.filter(|number| *number != 0) {
        let number = usize::from(number);
        for index in 0..number.min(voicing.slots.len()) {
            voicing.shift(index, 1);
            if number > 3 {
                voicing.shift(index, 1);
            }
        }
        // The bass is read as it stands now, for it may be in the list.
        for index in 0..voicing.slots.len() {
            let slot = voicing.slots[index];
            if voicing.pitch(slot).diatonic_note_number() < voicing.bass.diatonic_note_number() {
                voicing.shift(index, 1);
            }
        }
    }

    // Down while anything is above middle C's D, then back up while
    // anything is below the piano's lowest A: music21's `_hasPitchAboveC4`
    // and `_hasPitchBelowA1`.
    let highest = |voicing: &Voicing| {
        voicing
            .values()
            .iter()
            .map(Pitch::diatonic_note_number)
            .max()
    };
    while highest(&voicing).is_some_and(|number| number > 30) {
        voicing.shift_all(-1);
    }
    let lowest = |voicing: &Voicing| {
        voicing
            .values()
            .iter()
            .map(Pitch::diatonic_note_number)
            .min()
    };
    while lowest(&voicing).is_some_and(|number| number < 13) {
        voicing.shift_all(1);
    }

    let mut pitches = voicing.values();
    sort_diatonic(&mut pitches);
    // music21 then sets the bass on the chord it has built, and a bass the
    // chord has no note of by that name is put in front of it where it was
    // written: `C+/G` sounds a G under an augmented triad that has a G#.
    let bass = voicing.bass;
    if !pitches.iter().any(|pitch| pitch.name() == bass.name()) {
        pitches.insert(0, bass.clone());
    }
    Ok(Realized {
        pitches,
        root: voicing.root,
        bass,
    })
}

/// A degree token of the notation read as its number: `-7` and `#5` are the
/// seventh and the fifth.
fn degree_number(token: &str) -> Option<u8> {
    token.replace(['-', '#', 'A'], "").parse().ok()
}

/// music21's `_adjustPitchesForChordStepModifications`, over the list as the
/// steps before it left it.
///
/// Its loops remove from the lists they are walking, which in Python skips
/// the element that moves into the removed one's place; the walks here are
/// written to do the same, since which note a subtraction takes depends on
/// it.
fn apply_modifications(
    root: &Pitch,
    kind: &str,
    pitches: &mut Vec<Pitch>,
    degrees: &mut Vec<String>,
    modifications: &[ChordStepModification],
) -> Result<()> {
    let scale = Scale::new(ScaleType::Major, root.clone());
    for modification in modifications {
        match modification.modification_type {
            ChordStepModificationType::Add => {
                add_degree(&scale, root, kind, pitches, degrees, modification)?;
            }
            ChordStepModificationType::Subtract => {
                if degrees.is_empty() {
                    continue;
                }
                let mut found = false;
                let mut index = 0;
                while index < pitches.len() && index < degrees.len() {
                    if degree_number(&degrees[index]) == Some(modification.degree) {
                        let taken = pitches[index].clone();
                        if let Some(at) = pitches.iter().position(|pitch| *pitch == taken) {
                            let _ = pitches.remove(at);
                        }
                        found = true;
                        let written = modification.degree.to_string();
                        if let Some(at) = degrees.iter().position(|token| token.contains(&written))
                        {
                            let _ = degrees.remove(at);
                        }
                    }
                    index += 1;
                }
                if !found {
                    return Err(Error::Chord(format!(
                        "Degree not in specified chord: {}",
                        modification.degree
                    )));
                }
            }
            ChordStepModificationType::Alter => {
                let mut found = false;
                for index in 0..pitches.len().min(degrees.len()) {
                    if degree_number(&degrees[index]) == Some(modification.degree) {
                        pitches[index] = pitches[index].transpose(&modification.interval)?;
                        found = true;
                    }
                }
                // A degree the chord has not got is added instead, as
                // music21 turns the modification into an addition.
                if !found {
                    add_degree(&scale, root, kind, pitches, degrees, modification)?;
                }
            }
        }
    }
    Ok(())
}

/// music21's `typeAdd`: the degree of the major scale on the root, placed at
/// or above the root, moved by the modification's interval and put an
/// octave higher from the seventh up; in place of a note on the same degree
/// where the kind has one, and beside the others where it has not.
fn add_degree(
    scale: &Scale,
    root: &Pitch,
    kind: &str,
    pitches: &mut Vec<Pitch>,
    degrees: &[String],
    modification: &ChordStepModification,
) -> Result<()> {
    let folded = IntegerType::from((modification.degree.max(1) - 1) % 7 + 1);
    let mut added = scale.pitch_at_degree(folded)?;
    added.set_octave(Some(octave_of(root)));
    if added.ps() < root.ps() {
        shift_octave(&mut added, 1);
    }
    let semitones = modification.interval.semitones();
    if semitones != 0.0 {
        // Added degrees are measured from a dominant chord, whose seventh is
        // minor, so a raised seventh is raised from there: a semitone down
        // first, spelled from its number as music21's integer transposition
        // spells it.
        if modification.degree == 7 && semitones > 0.0 && !kind.is_empty() {
            let mut lowered = Pitch::from_number(added.ps() - 1.0)?;
            lowered.set_octave(Some(octave_of(&lowered)));
            added = lowered;
        }
        added = added.transpose(&modification.interval)?;
    }
    if modification.degree >= 7 {
        shift_octave(&mut added, 1);
    }
    if degrees.contains(&modification.degree.to_string()) {
        let mut index = 0;
        while index < pitches.len() {
            let on_degree = scale.degree_of(&pitches[index])?;
            if on_degree == Some(usize::from(modification.degree)) {
                let _ = pitches.remove(index);
                pitches.push(added.clone());
            }
            index += 1;
        }
    } else {
        pitches.push(added);
    }
    Ok(())
}
