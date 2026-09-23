//! A scale realized from a tonic: music21's `ConcreteScale`, the pitches
//! a pattern of steps gives on a note, and every question asked of them —
//! degrees, neighbours, matching, derivation and tuning.

use super::scaletype::{
    DegreeComparison, HUMDRUM_SOLFEG_SYLLABLES, MAX_RANGE_OCTAVES, SCALE_STARTS, SOLFEG_SYLLABLES,
    ScaleType, Simplification, SolfegVariant, advance, step_interval,
};
use crate::chord::{Chord, root};
use crate::defaults::{FloatType, IntegerType};
use crate::error::{Error, Result};
use crate::interval::Interval;
use crate::key::Key;
use crate::pitch::Pitch;
use crate::roman::{Minor67Default, RomanNumeral, degree_to_roman};
use crate::tuningsystem::scala::{ScalaDegree, ScalaScale};

/// A named scale realized from a tonic pitch.
///
/// ```
/// use music21_rs::{Pitch, Scale, ScaleType};
///
/// let scale = Scale::new(ScaleType::Octatonic, Pitch::from_name("C4")?);
/// let names: Vec<String> = scale.pitches()?.iter().map(|p| p.name()).collect();
///
/// assert_eq!(names, ["C", "D", "E-", "F", "G-", "A-", "A", "B", "C"]);
/// # Ok::<(), music21_rs::Error>(())
/// ```
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[must_use]
pub struct Scale {
    scale_type: ScaleType,
    tonic: Pitch,
    /// The steps of a scale nobody has a name for.
    ///
    /// music21's `ConcreteScale(pitches=[...])` is a scale given by its
    /// notes rather than by a name, and it behaves as any other scale does —
    /// it realizes, it has degrees, it can be matched against. `None` is the
    /// ordinary case, where the steps come from the named type.
    ///
    /// They are intervals and not names: a step between microtonal pitches
    /// has no name to be written and read back through.
    #[cfg_attr(feature = "serde", serde(default))]
    custom_steps: Option<Vec<Interval>>,
    /// How a scale given by its steps spells what it realizes, where that is
    /// not how the named type would. `None` leaves it to the type.
    #[cfg_attr(feature = "serde", serde(default))]
    custom_simplification: Option<Simplification>,
    /// Whether the scale is a cycle, realized by walking out from its tonic
    /// both ways rather than once and moved by octaves: music21's
    /// `AbstractCyclicalScale`, whose network does not duplicate at the
    /// octave even where its steps happen to add up to one.
    #[cfg_attr(feature = "serde", serde(default))]
    cyclic: bool,
}

impl Scale {
    /// The roman numeral on a degree of this scale: music21's `romanNumeral`,
    /// the triad that degree carries read against the major key of the
    /// tonic, whatever its case would say.
    pub fn roman_numeral(&self, degree: u8) -> Result<RomanNumeral> {
        if !(1..=7).contains(&degree) {
            return Err(Error::Scale(format!(
                "a roman numeral stands on a degree from 1 to 7, not {degree}"
            )));
        }
        let key = Key::from_tonic_mode(&self.tonic.name(), "major")?;
        RomanNumeral::over_scale(
            degree_to_roman(degree),
            key,
            Some(self.clone()),
            Minor67Default::default(),
            Minor67Default::default(),
            false,
        )
    }

    /// Moves every note and chord of a stream onto this scale: music21's
    /// `tune`. A pitch whose name, or any enharmonic of it within two
    /// accidentals, is a name of the scale's octave from the tonic becomes
    /// that scale pitch in its own octave, spelled as it was where the scale
    /// pitch has such a spelling; a pitch the scale has no name for is left
    /// alone. Nested streams are tuned too.
    pub fn tune(&self, stream: &mut crate::Stream) -> Result<()> {
        let scale_pitches = self.pitches()?;
        let names: Vec<String> = scale_pitches.iter().map(Pitch::name).collect();
        let tuned = |pitch: &Pitch| -> Result<Option<Pitch>> {
            let mut candidates = pitch.all_common_enharmonics(2);
            candidates.push(pitch.clone());
            for candidate in candidates {
                let Some(index) = names.iter().position(|name| *name == candidate.name()) else {
                    continue;
                };
                let mut target = scale_pitches[index].clone();
                target.set_octave(candidate.octave());
                let spelled = target
                    .all_common_enharmonics(2)
                    .into_iter()
                    .find(|spelling| spelling.name() == pitch.name());
                return Ok(Some(spelled.unwrap_or(target)));
            }
            Ok(None)
        };
        for event in stream.events_mut() {
            match event.element_mut() {
                crate::stream::StreamElement::Note(note) => {
                    if let Some(pitch) = tuned(note.pitch())? {
                        note.set_pitch(pitch);
                    }
                }
                crate::stream::StreamElement::Chord(chord) => {
                    for note in chord.notes_mut() {
                        if let Some(pitch) = tuned(note.pitch())? {
                            note.set_pitch(pitch);
                        }
                    }
                }
                crate::stream::StreamElement::Stream(inner) => self.tune(inner)?,
                _ => {}
            }
        }
        Ok(())
    }

    /// This scale written as a Scala file writes one: music21's
    /// `getScalaData`, each degree as the cents above the tonic, with the
    /// interval the scale closes on as the period.
    pub fn scala_data(&self) -> Result<ScalaScale> {
        let pitches = self.pitches()?;
        let (Some(tonic), Some(closing)) = (pitches.first(), pitches.last()) else {
            return Err(Error::Scale(
                "a scale with no pitches has no degrees".to_string(),
            ));
        };
        let cents = |pitch: &Pitch| ScalaDegree::Cents((pitch.ps() - tonic.ps()) * 100.0);
        let mut degrees = vec![ScalaDegree::Ratio(crate::tuningsystem::Fraction::new(1, 1))];
        degrees.extend(pitches[1..pitches.len() - 1].iter().map(cents));
        Ok(ScalaScale::new(
            format!(
                "{} {}",
                self.tonic.name(),
                self.scale_type.music21_descriptive_name()
            ),
            degrees,
            cents(closing),
        ))
    }

    /// Builds a scale of the given type on a tonic.
    pub fn new(scale_type: ScaleType, tonic: Pitch) -> Self {
        Self {
            scale_type,
            tonic,
            custom_steps: None,
            custom_simplification: None,
            cyclic: false,
        }
    }

    /// A scale given by the notes of one octave of it rather than by a name:
    /// music21's `ConcreteScale(pitches=[...])`.
    ///
    /// The first pitch is the tonic, the steps are the intervals between
    /// neighbours, and the scale closes back on the octave, so the notes
    /// repeat an octave higher as any scale's do.
    pub fn from_pitches(pitches: &[Pitch]) -> Result<Self> {
        let pitches = rising_octaves(pitches);
        let pitches = pitches.as_slice();
        let Some(tonic) = pitches.first() else {
            return Err(crate::error::Error::Scale(
                "a scale needs at least one pitch".to_string(),
            ));
        };
        let mut steps = Vec::with_capacity(pitches.len());
        for pair in pitches.windows(2) {
            steps.push(Interval::between_pitches(&pair[0], &pair[1])?);
        }
        // The closing step back to the tonic, so the collection repeats.
        // Notes that already close on it need none — and they may close two
        // octaves up rather than one, which is a pattern two octaves long and
        // not a scale that folds back on itself.
        let last = pitches.last().unwrap_or(tonic);
        let span = last.ps() - tonic.ps();
        if span.rem_euclid(12.0) != 0.0 {
            let octaves = (span / 12.0).floor() + 1.0;
            let closing = tonic.transpose(&Interval::from_semitones(
                (octaves * 12.0) as crate::defaults::IntegerType,
            )?)?;
            steps.push(Interval::between_pitches(last, &closing)?);
        }
        Ok(Self {
            scale_type: ScaleType::Major,
            tonic: tonic.clone(),
            custom_steps: Some(steps),
            custom_simplification: None,
            cyclic: false,
        })
    }

    /// A scale given by the steps between its notes rather than by a name or
    /// by the notes themselves: the pattern music21's `OctaveRepeatingScale`
    /// and `CyclicalScale` hand their interval network.
    ///
    /// The steps are walked in order from the tonic and then again from
    /// wherever they leave off, so they are one period of the scale. A
    /// pattern with no steps in it is no scale at all.
    pub fn from_steps(tonic: Pitch, steps: Vec<Interval>) -> Result<Self> {
        if steps.is_empty() {
            return Err(crate::error::Error::Scale(
                "a scale needs at least one step".to_string(),
            ));
        }
        Ok(Self {
            scale_type: ScaleType::Major,
            tonic,
            custom_steps: Some(steps),
            // music21's interval network spells with at most one accidental
            // unless it is told otherwise, which is what turns the third
            // minor second above C from E double flat into D.
            custom_simplification: Some(Simplification::MaxAccidental),
            cyclic: false,
        })
    }

    /// A scale that is a cycle of steps from its tonic: music21's
    /// `CyclicalScale`, `SieveScale` and `ScalaScale`, all of which stand on
    /// an `AbstractCyclicalScale`.
    ///
    /// It sounds the same notes [`Self::from_steps`] would where the steps
    /// add up to an octave, but it reaches each one by walking to it from
    /// the tonic, so a note below the tonic is spelled for the step it was
    /// reached by coming down. With microtones that is a different spelling
    /// -- slendro on C has `B-3(-40c)` below its tonic and `A~4(+10c)` above.
    pub fn from_cycle(tonic: Pitch, steps: Vec<Interval>) -> Result<Self> {
        let mut scale = Self::from_steps(tonic, steps)?;
        scale.cyclic = true;
        Ok(scale)
    }

    /// The same scale spelling what it realizes another way: music21's
    /// `pitchSimplification` set on the network underneath. It changes how
    /// the notes are written and nothing about where they sound.
    pub fn with_simplification(mut self, simplification: Simplification) -> Self {
        self.custom_simplification = Some(simplification);
        self
    }

    /// A scale read out of a Scala file: music21's `ScalaScale`, the file's
    /// steps walked from a tonic, each note written in its most common
    /// spelling as music21 writes them.
    ///
    /// The steps are the file's own, in cents, and the cycle closes on the
    /// file's last degree, so a scale that does not repeat at the octave does
    /// not here either.
    pub fn from_scala(tonic: Pitch, scala: &ScalaScale) -> Result<Self> {
        Ok(Self::from_cycle(tonic, scala.interval_sequence()?)?
            .with_simplification(Simplification::MostCommon))
    }

    /// This scale as it sounds coming down, which for most is itself.
    pub fn descending(&self) -> Scale {
        if self.custom_steps.is_some() {
            return self.clone();
        }
        if let Some(scale_type) = self.scale_type.descending_form() {
            return Scale::new(scale_type, self.tonic.clone());
        }
        // A pattern no other named scale spells, walked as its own list of
        // steps: Rag Marwa comes down through the flat second above its
        // octave and no scale here rises that way.
        if let Some(steps) = self.scale_type.descending_steps() {
            let walked: Result<Vec<Interval>> = steps.iter().copied().map(step_interval).collect();
            if let Ok(walked) = walked {
                return Self {
                    scale_type: self.scale_type,
                    tonic: self.tonic.clone(),
                    custom_steps: Some(walked),
                    custom_simplification: None,
                    cyclic: false,
                };
            }
        }
        self.clone()
    }

    /// The scale coming down: highest note first, through the collection it
    /// uses descending.
    pub fn pitches_descending(&self) -> Result<Vec<Pitch>> {
        let mut pitches = self.descending().pitches()?;
        pitches.reverse();
        Ok(pitches)
    }

    /// A range of the scale coming down, highest note first.
    pub fn pitches_between_descending(
        &self,
        minimum: &Pitch,
        maximum: &Pitch,
    ) -> Result<Vec<Pitch>> {
        if self.custom_simplification.is_some() {
            return self.walked_down_between(minimum, maximum);
        }
        let mut pitches = self.descending().pitches_between(minimum, maximum)?;
        pitches.reverse();
        Ok(pitches)
    }

    /// A range of a scale given by its steps, walked downward from the tonic
    /// above it.
    ///
    /// Which way a scale is walked only matters where it respells as it
    /// goes, and then it matters: three minor seconds up from D are `E-`,
    /// `F-` and `F`, and the same three coming down from F are `E`, `D#` and
    /// `D`. music21 walks its network in the direction it is asked for, so
    /// the notes coming down are not the notes going up read backwards.
    fn walked_down_between(&self, minimum: &Pitch, maximum: &Pitch) -> Result<Vec<Pitch>> {
        let (lowest, highest) = (
            minimum.ps().min(maximum.ps()),
            minimum.ps().max(maximum.ps()),
        );
        if !self.repeats_at_the_octave() || self.cyclic {
            let mut pitches: Vec<Pitch> = self
                .cycle_between(lowest, highest)?
                .into_iter()
                .map(|(pitch, _)| pitch)
                .collect();
            pitches.reverse();
            return Ok(pitches);
        }
        let simplification = self.simplification();
        let steps = self.walk()?;
        let period = self.period_in_octaves(&steps);
        let mut current = self.realization_start()?;
        while current.ps() < highest {
            let octave = current.octave().unwrap_or(0);
            current.octave_setter(Some(octave + period));
        }
        let mut pitches = Vec::new();
        let limit = steps.len() * (MAX_RANGE_OCTAVES + 2) + 1;
        let clear_of = lowest - 12.0 * FloatType::from(period);
        for index in 0..limit {
            let sounding = current.ps();
            if sounding < clear_of {
                break;
            }
            if within(sounding, lowest, highest) {
                pitches.push(current.clone());
            }
            let step = &steps[steps.len() - 1 - index % steps.len()];
            current = advance(&current, &step.reversed()?, simplification)?;
        }
        Ok(pitches)
    }

    /// The steps of a scale with no name, in the order they are walked: a
    /// cycle a caller gave, or the steps between the notes a caller gave.
    /// `None` for a named scale, whose steps are its type's.
    pub fn custom_steps(&self) -> Option<&[Interval]> {
        self.custom_steps.as_deref()
    }

    /// Whether this scale is a cycle walked from its tonic: see
    /// [`Self::from_cycle`].
    pub fn is_cyclic(&self) -> bool {
        self.cyclic
    }

    /// Whether this scale was given by its notes rather than by a name.
    pub fn is_custom(&self) -> bool {
        self.custom_steps.is_some()
    }

    /// Moves the scale to a new tonic, keeping its pattern of steps.
    pub fn set_tonic(&mut self, tonic: Pitch) {
        self.tonic = tonic;
    }

    /// The scales of *this* pattern that contain the most of `pitches`, best
    /// first: music21's `deriveRanked` on the scale rather than on the type,
    /// which is what a scale given by its notes has to use.
    pub fn derive_ranked_by(
        &self,
        pitches: &[Pitch],
        limit: Option<usize>,
        comparison: DegreeComparison,
    ) -> Result<Vec<(usize, Scale)>> {
        if !self.is_custom() {
            return self.scale_type.derive_ranked_by(pitches, limit, comparison);
        }
        let targets: Vec<String> = pitches.iter().map(|p| comparison.key(p)).collect();
        let mut ranked = Vec::with_capacity(SCALE_STARTS.len());
        for start in SCALE_STARTS {
            let mut candidate = self.clone();
            candidate.set_tonic(Pitch::from_name(start)?);
            let degrees: Vec<String> = candidate
                .pitches()?
                .iter()
                .map(|p| comparison.key(p))
                .collect();
            let matched = targets
                .iter()
                .filter(|target| degrees.contains(target))
                .count();
            ranked.push((matched, candidate));
        }
        ranked.sort_by(|left, right| {
            left.0
                .cmp(&right.0)
                .then_with(|| left.1.tonic().ps().total_cmp(&right.1.tonic().ps()))
        });
        ranked.reverse();
        if let Some(limit) = limit {
            ranked.truncate(limit);
        }
        Ok(ranked)
    }

    /// How the scale spells what it realizes: music21's
    /// `pitchSimplification` on the network underneath.
    pub fn simplification(&self) -> Simplification {
        self.custom_simplification
            .unwrap_or_else(|| self.scale_type.simplification())
    }

    /// The steps walked from where the scale is realized.
    fn walk(&self) -> Result<Vec<Interval>> {
        match &self.custom_steps {
            Some(steps) => Ok(steps.clone()),
            None => self
                .scale_type
                .realization_steps()
                .into_iter()
                .map(step_interval)
                .collect(),
        }
    }

    /// Returns the number of distinct degrees: music21's `getDegreeMaxUnique`,
    /// seven for a major scale and twelve for the chromatic.
    pub fn degree_count(&self) -> usize {
        match &self.custom_steps {
            Some(steps) => steps.len(),
            None => self.scale_type.degree_count(),
        }
    }

    /// Returns the scale type.
    pub fn scale_type(&self) -> ScaleType {
        self.scale_type
    }

    /// Returns the tonic pitch.
    pub fn tonic(&self) -> &Pitch {
        &self.tonic
    }

    /// Returns the pitches of one octave, from the tonic through its octave.
    ///
    /// The result has `degree_count() + 1` entries, since the closing octave is
    /// included the way music21's `getPitches` includes it.
    ///
    /// A tonic with no octave is realized in octave 4, which is what music21
    /// does: the scale on a bare `G` runs `G4 A4 B-4 C5 …`. The tonic itself
    /// keeps its own spelling — `Scale::tonic` still has no octave — because
    /// the octave belongs to the realization and not to the scale.
    pub fn pitches(&self) -> Result<Vec<Pitch>> {
        let simplification = self.simplification();
        let start = self.realization_start()?;
        let mut pitches = Vec::with_capacity(self.scale_type.degree_count() + 1);
        pitches.push(start.clone());

        let mut current = start;
        for step in self.walk()? {
            current = advance(&current, &step, simplification)?;
            pitches.push(current.clone());
        }
        if self.custom_steps.is_none()
            && let Some(beyond) = self.scale_type.beyond_terminus()
        {
            pitches.push(advance(&current, &step_interval(beyond)?, simplification)?);
        }
        Ok(pitches)
    }

    /// The pitch the scale is realized from, which is its final except in a
    /// plagal mode, where the range starts below it.
    ///
    /// How far below is the scale's own business rather than a flat fourth:
    /// the walk goes back down the last steps of the collection, so the
    /// hypolocrian on C starts on `G-` and not on `G`, its fifth degree
    /// being diminished.
    fn realization_start(&self) -> Result<Pitch> {
        let mut start = self.realized_tonic();
        if self.custom_steps.is_some() {
            return Ok(start);
        }
        let steps = self.scale_type.steps();
        for step in steps
            .iter()
            .rev()
            .take(self.scale_type.tonic_degree().saturating_sub(1))
        {
            start = start.transpose(&step_interval(step)?.reversed()?)?;
        }
        Ok(start)
    }

    /// The tonic as the scale sounds it: music21's `getTonic`, which is the
    /// tonic in octave 4 when it was given without one.
    pub fn realized_tonic(&self) -> Pitch {
        let mut tonic = self.tonic.clone();
        if tonic.octave().is_none() {
            tonic.octave_setter(Some(crate::defaults::PITCH_OCTAVE as IntegerType));
        }
        tonic
    }

    /// The major scale written with the same key signature as this one:
    /// music21's `getRelativeMajor`.
    ///
    /// A mode is written with the signature of the major scale it is a
    /// rotation of, so D dorian is C major and E minor is G major. Only the
    /// seven-note modes have one.
    pub fn relative_major(&self) -> Result<Scale> {
        self.relative(ScaleType::Major)
    }

    /// The minor scale written with the same key signature: music21's
    /// `getRelativeMinor`.
    pub fn relative_minor(&self) -> Result<Scale> {
        self.relative(ScaleType::Minor)
    }

    /// The major scale on the same tonic: music21's `getParallelMajor`.
    pub fn parallel_major(&self) -> Scale {
        Scale::new(ScaleType::Major, self.tonic.clone())
    }

    /// The minor scale on the same tonic: music21's `getParallelMinor`.
    pub fn parallel_minor(&self) -> Scale {
        Scale::new(ScaleType::Minor, self.tonic.clone())
    }

    /// The scale of the wanted type carrying this one's key signature.
    ///
    /// It stands at or above this scale, in the same octave where that is
    /// possible: the relative major of A minor on `A4` is C major on `C5`,
    /// because a `C4` would sound below the scale it came from.
    fn relative(&self, wanted: ScaleType) -> Result<Scale> {
        let mode = self.scale_type.music21_descriptive_name();
        let sharps = crate::key::pitch_to_sharps(&self.tonic, Some(mode))?;
        let key = crate::key::KeySignature::new(sharps)
            .try_as_key(Some(wanted.music21_descriptive_name()), None)?;
        let here = self.realized_tonic();
        let mut tonic = key.tonic();
        tonic.set_octave(here.octave());
        if tonic.ps() < here.ps() {
            tonic.set_octave(tonic.octave().map(|octave| octave + 1));
        }
        Ok(Scale::new(wanted, tonic))
    }

    /// The degree each of this scale's notes stands on, in order from the
    /// tonic, where those are not simply the notes counted off.
    ///
    /// Only a named pattern can say so — a collection given by its notes is
    /// counted — and only one of them does: see
    /// [`ScaleType::ascending_degrees`].
    pub fn named_degrees(&self) -> Option<Vec<IntegerType>> {
        if self.custom_steps.is_some() {
            return None;
        }
        Some(
            self.scale_type
                .ascending_degrees()?
                .iter()
                .map(|&degree| IntegerType::from(degree))
                .collect(),
        )
    }

    /// The degree the note at this position stands on, counting from one.
    fn degree_at_position(&self, position: usize) -> usize {
        self.named_degrees()
            .and_then(|degrees| degrees.get(position).copied())
            .map_or(position + 1, |degree| degree as usize)
    }

    /// Returns the pitch standing on a one-based scale degree, or nothing
    /// where the scale has no such degree.
    ///
    /// Degree 1 is the tonic. Every other degree is read within the one
    /// octave the scale is realized in, so the eighth degree is the tonic
    /// again and not the octave above it, and the zeroth and the negative
    /// degrees count back round from the top. That is music21's
    /// `pitchFromDegree`, which asks its interval network for the node the
    /// degree names and gets one of the nodes it has.
    ///
    /// A scale that names its degrees has only the ones it names: Rag
    /// Asawari's ascent has no third, and answers nothing when asked for
    /// one rather than handing back the note that would be third in line.
    pub fn pitch_on_degree(&self, degree: IntegerType) -> Result<Option<Pitch>> {
        let position = match self.named_degrees() {
            Some(degrees) => match degrees.iter().position(|&named| named == degree) {
                Some(position) => position,
                None => return Ok(None),
            },
            None => {
                let count = self.degree_count().max(1) as IntegerType;
                (degree - 1).rem_euclid(count) as usize
            }
        };
        let simplification = self.simplification();
        let steps = self.walk()?;
        let mut current = self.realization_start()?;
        for index in 0..position {
            current = advance(&current, &steps[index % steps.len()], simplification)?;
        }
        Ok(Some(current))
    }

    /// The pitch standing on a one-based scale degree, which the scale is
    /// expected to have: [`Self::pitch_on_degree`] is the one that says when
    /// it does not.
    pub fn pitch_at_degree(&self, degree: IntegerType) -> Result<Pitch> {
        self.pitch_on_degree(degree)?.ok_or_else(|| {
            Error::Scale(format!(
                "{} has no degree {degree}",
                self.scale_type.music21_descriptive_name()
            ))
        })
    }
    /// Returns every pitch of the scale from `minimum` up to `maximum`,
    /// inclusive: music21's `getPitches` given a range.
    ///
    /// The scale is realized from the tonic in whatever octave puts it at or
    /// below the bottom of the range, then walked upward, so asking a C major
    /// scale for `E-5` to `G-7` starts at `E5` — the first scale pitch that
    /// is not below the bottom — and not at a respelled `E-5`.
    pub fn pitches_between(&self, minimum: &Pitch, maximum: &Pitch) -> Result<Vec<Pitch>> {
        // Asked the other way round, music21 walks down instead: the same
        // pitches, highest first.
        if maximum.ps() < minimum.ps() {
            let mut descending = self.pitches_between(maximum, minimum)?;
            descending.reverse();
            return Ok(descending);
        }
        Ok(self
            .placed_between(minimum.ps(), maximum.ps())?
            .into_iter()
            .map(|(pitch, _)| pitch)
            .collect())
    }

    /// The pitch standing on a degree within a range: the first one the
    /// scale comes to walking up from the bottom of it, or nothing where no
    /// note of that degree falls inside. music21's `pitchFromDegree` given a
    /// `minPitch` and a `maxPitch`, which realizes the range and reads the
    /// degree off it — so a cycle of minor seconds on C4 asked for its first
    /// degree between C2 and C3 answers the `B#1` it spells there.
    pub fn pitch_on_degree_between(
        &self,
        degree: IntegerType,
        minimum: &Pitch,
        maximum: &Pitch,
    ) -> Result<Option<Pitch>> {
        let (low, high) = if maximum.ps() < minimum.ps() {
            (maximum, minimum)
        } else {
            (minimum, maximum)
        };
        let count = self.degree_count().max(1);
        let wanted = |place: usize| match self.named_degrees() {
            Some(_) => self.degree_at_position(place) as IntegerType == degree,
            None => place % count == (degree - 1).rem_euclid(count as IntegerType) as usize,
        };
        Ok(self
            .placed_between(low.ps(), high.ps())?
            .into_iter()
            .find(|(_, place)| wanted(*place))
            .map(|(pitch, _)| pitch))
    }

    /// Every note of the scale sounding between two pitch-space values, in
    /// the order the walk comes to them, each with its place in the pattern.
    fn placed_between(&self, lowest: FloatType, highest: FloatType) -> Result<Vec<(Pitch, usize)>> {
        if !self.repeats_at_the_octave() || self.cyclic {
            return self.cycle_between(lowest, highest);
        }
        let simplification = self.simplification();
        let steps = self.walk()?;
        // Down whole periods until the start is at or below the range. A
        // period is usually the octave, but a scale given by its notes may
        // take two to come back to where it began, and dropping by one would
        // start the pattern halfway through itself.
        let period = self.period_in_octaves(&steps);
        let mut current = self.realization_start()?;
        while current.ps() > lowest {
            let octave = current.octave().unwrap_or(0);
            current.octave_setter(Some(octave - period));
        }
        let mut pitches = Vec::new();
        // Two octaves of headroom past the range, so a scale whose degrees
        // are not evenly spaced still reaches the top of it.
        let limit = steps.len() * (MAX_RANGE_OCTAVES + 2) + 1;
        // A pattern may rise above the range and fall back into it — Rag
        // Marwa's descending form goes up to the flat second above its
        // octave and closes on the octave below that — so the walk carries
        // on until it is clear of the range by a whole period.
        let clear_of = highest + 12.0 * FloatType::from(period);
        for index in 0..limit {
            let sounding = current.ps();
            if sounding > clear_of {
                break;
            }
            if within(sounding, lowest, highest) {
                pitches.push((current.clone(), index % steps.len()));
            }
            current = advance(&current, &steps[index % steps.len()], simplification)?;
        }
        Ok(pitches)
    }

    /// Whether the pattern comes back to its tonic a whole number of octaves
    /// up, as every named scale does. A cycle of fifths does not: it comes
    /// back a fifth up, and again a fifth above that, and never on an octave
    /// of where it began until it has been all the way round.
    fn repeats_at_the_octave(&self) -> bool {
        match &self.custom_steps {
            Some(steps) => {
                let risen: FloatType = steps.iter().map(Interval::semitones).sum();
                (risen / 12.0 - (risen / 12.0).round()).abs() < 1e-9
            }
            None => true,
        }
    }

    /// The notes of a cycle sounding between two pitch-space values, lowest
    /// first, each with its place in the cycle.
    ///
    /// A scale that repeats at the octave can be realized once and moved by
    /// octaves; one that does not has to be walked, up from the tonic for
    /// what lies above it and down from the tonic for what lies below, which
    /// is also how each note comes by its spelling.
    fn cycle_between(&self, lowest: FloatType, highest: FloatType) -> Result<Vec<(Pitch, usize)>> {
        let steps = self.walk()?;
        let period: FloatType = steps.iter().map(Interval::semitones).sum();
        if period <= 0.0 {
            return Err(Error::Scale(
                "a cycle that does not rise cannot be walked over a range".to_string(),
            ));
        }
        let simplification = self.simplification();
        let start = self.realization_start()?;
        let limit = steps.len() * 512;
        let mut found = Vec::new();

        // Each walk stops at the first note past its end of the range, as
        // music21's does. Going further is not only wasted: a Scala file's
        // steps respell as they go, and a walk carried a cycle too far can
        // land on a note no accidental spells, which music21 never reaches.
        let mut current = start.clone();
        for index in 0..limit {
            let sounding = current.ps();
            if within(sounding, lowest, highest) {
                found.push((current.clone(), index % steps.len()));
            }
            if sounding > highest - SLACK {
                break;
            }
            current = advance(&current, &steps[index % steps.len()], simplification)?;
        }

        let mut current = start;
        for back in 1..=limit {
            let place = (steps.len() - back % steps.len()) % steps.len();
            current = advance(&current, &steps[place].reversed()?, simplification)?;
            let sounding = current.ps();
            if within(sounding, lowest, highest) {
                found.push((current.clone(), place));
            }
            if sounding < lowest + SLACK {
                break;
            }
        }
        found.sort_by(|left, right| left.0.ps().total_cmp(&right.0.ps()));
        Ok(found)
    }

    /// Every place a pitch stands on in a cycle that does not repeat at the
    /// octave, lowest first.
    ///
    /// Such a cycle need not have a note in every register — a cycle of
    /// fifths on C has an F#, but not the one below middle C — so music21
    /// realizes the cycle an octave either side of the pitch and takes every
    /// note there that matches it. A pitch with no octave is heard in the
    /// fourth. Two matches may stand on different places, and music21
    /// chooses between them at random; which there are is answered here.
    fn cycle_places_of(&self, pitch: &Pitch, comparison: DegreeComparison) -> Result<Vec<usize>> {
        let sounding = Self::sounding(pitch).ps();
        let wanted = comparison.key(pitch);
        let mut places = Vec::new();
        for (candidate, place) in self.cycle_between(sounding - 12.0, sounding + 12.0)? {
            if comparison.key(&candidate) == wanted && !places.contains(&place) {
                places.push(place);
            }
        }
        Ok(places)
    }

    /// The note of the cycle sounding exactly where a pitch does, which is
    /// what it takes for the pitch to be on the cycle and not merely named
    /// by it somewhere.
    fn cycle_place_sounding(
        &self,
        pitch: &Pitch,
        comparison: DegreeComparison,
    ) -> Result<Option<(Pitch, usize)>> {
        let sounding = Self::sounding(pitch).ps();
        let wanted = comparison.key(pitch);
        Ok(self
            .cycle_between(sounding - 1e-9, sounding + 1e-9)?
            .into_iter()
            .find(|(candidate, _)| comparison.key(candidate) == wanted))
    }

    /// A pitch as a scale hears it, in octave four where it names none.
    fn sounding(pitch: &Pitch) -> Pitch {
        let mut heard = pitch.clone();
        if heard.octave().is_none() {
            heard.octave_setter(Some(crate::defaults::PITCH_OCTAVE as IntegerType));
        }
        heard
    }

    /// `steps` notes along a cycle from a pitch, which need not be on it: a
    /// pitch beside the cycle first comes onto it, on the side the move is
    /// going or the one named, and that counts as a step unless a side was
    /// named.
    fn cycle_steps_from(
        &self,
        origin: &Pitch,
        steps: IntegerType,
        neighbour_below: Option<bool>,
    ) -> Result<Pitch> {
        let walk = self.walk()?;
        let period: FloatType = walk.iter().map(Interval::semitones).sum();
        let simplification = self.simplification();
        let at = Self::sounding(origin).ps();
        let ascending = steps > 0;
        let (mut current, mut place, mut remaining) =
            match self.cycle_place_sounding(origin, DegreeComparison::Name)? {
                Some((pitch, place)) => (pitch, place, steps),
                None => {
                    let take_below = neighbour_below.unwrap_or(!ascending);
                    let beside = self.cycle_between(at - period, at + period)?;
                    let neighbour = if take_below {
                        beside.into_iter().rfind(|(pitch, _)| pitch.ps() < at)
                    } else {
                        beside.into_iter().find(|(pitch, _)| pitch.ps() > at)
                    };
                    let (pitch, place) = neighbour
                        .ok_or_else(|| Error::Scale(format!("no scale pitch beside {origin}")))?;
                    let remaining = if neighbour_below.is_some() {
                        steps
                    } else {
                        steps - steps.signum()
                    };
                    (pitch, place, remaining)
                }
            };
        while remaining > 0 {
            current = advance(&current, &walk[place], simplification)?;
            place = (place + 1) % walk.len();
            remaining -= 1;
        }
        while remaining < 0 {
            place = (place + walk.len() - 1) % walk.len();
            current = advance(&current, &walk[place].reversed()?, simplification)?;
            remaining += 1;
        }
        if origin.octave().is_none() {
            current.octave_setter(None);
        }
        Ok(current)
    }

    /// Whether the pattern can be walked at all.
    ///
    /// A collection given by its notes may rise and fall back to where it
    /// began — `A4 B4 C4 D4 E4 F4 G4 A4` does — and a pattern that goes
    /// nowhere cannot be realized over a range, however many times it is
    /// walked. music21 says so as well, out of the network it walks.
    pub fn is_realizable(&self) -> bool {
        match &self.custom_steps {
            Some(steps) => steps.iter().map(Interval::semitones).sum::<FloatType>() > 0.0,
            None => true,
        }
    }

    /// Whether the pattern repeats at the octave: music21's
    /// `octaveDuplicating`. Every named scale does, and one given by its
    /// notes need not — a collection spanning two octaves before it comes
    /// back to its tonic is a pattern two octaves long.
    pub fn octave_duplicating(&self) -> bool {
        if self.cyclic {
            return false;
        }
        match &self.custom_steps {
            Some(steps) => self.period_in_octaves(steps) == 1,
            None => true,
        }
    }

    /// How many octaves the pattern takes to come back to where it began,
    /// which is one for every scale that has a name and may be more for one
    /// given by its notes.
    fn period_in_octaves(&self, steps: &[Interval]) -> IntegerType {
        let semitones: FloatType = steps.iter().map(Interval::semitones).sum();
        ((semitones / 12.0).round() as IntegerType).max(1)
    }

    /// The note the scale comes to rest on, as it sounds: music21's
    /// `getTonic`, which is the fourth degree of a plagal mode.
    pub fn final_pitch(&self) -> Result<Pitch> {
        self.pitch_at_degree(self.scale_type.tonic_degree() as IntegerType)
    }

    /// The reciting tone: music21's `getDominant`.
    pub fn dominant(&self) -> Result<Pitch> {
        self.pitch_at_degree(self.scale_type.dominant_degree() as IntegerType)
    }

    /// The seventh degree raised or lowered to sit a semitone below the
    /// final: music21's `getLeadingTone`, which in a minor scale is not the
    /// seventh degree the scale itself has.
    pub fn leading_tone(&self) -> Result<Pitch> {
        let seventh = self.pitch_at_degree(7)?;
        let tonic = self.final_pitch()?;
        let distance = seventh.midi() - tonic.midi();
        if distance == 11 {
            return Ok(seventh);
        }
        let alter = seventh.accidental_or_natural().alter() + FloatType::from(11 - distance);
        let mut raised = seventh.clone();
        raised.set_accidental(Some(crate::pitch::Accidental::new(alter)?));
        Ok(raised)
    }

    /// Returns the scale of the same type on which `pitch` is the given degree:
    /// music21's `deriveByDegree`, so the major scale with `E` as its fifth is
    /// A major. The pitch keeps its spelling; a pitch without an octave is
    /// read in octave 4, as music21 reads it, so the new tonic has one.
    pub fn derive_by_degree(&self, degree: usize, pitch: &Pitch) -> Result<Scale> {
        let implicit_octave = Some(crate::defaults::PITCH_OCTAVE as IntegerType);
        let mut tonic = self.tonic.clone();
        if tonic.octave().is_none() {
            tonic.octave_setter(implicit_octave);
        }
        let degree_pitch =
            Scale::new(self.scale_type, tonic.clone()).pitch_at_degree(degree as IntegerType)?;
        let up_to_degree = Interval::between_pitches(&tonic, &degree_pitch)?;
        let mut reference = pitch.clone();
        if reference.octave().is_none() {
            reference.octave_setter(implicit_octave);
        }
        let new_tonic = reference.transpose(&up_to_degree.reversed()?)?;
        Ok(Scale::new(self.scale_type, new_tonic))
    }

    /// Returns the same scale type on the tonic transposed by `interval`.
    pub fn transpose(&self, interval: &Interval) -> Result<Scale> {
        Ok(Scale::new(self.scale_type, self.tonic.transpose(interval)?))
    }

    /// Returns one octave of the scale as a chord, tonic through octave:
    /// music21's `getChord`.
    pub fn chord(&self) -> Result<Chord> {
        Chord::new(self.pitches()?.as_slice())
    }

    /// Returns the pitches at the given degrees within one octave of the
    /// tonic: music21's `pitchesFromScaleDegrees`, which realizes tonic
    /// through octave once and so silently drops a degree beyond the octave.
    pub fn pitches_from_scale_degrees(&self, degrees: &[usize]) -> Result<Vec<Pitch>> {
        let octave = self.pitches()?;
        // The realization closes on the tonic an octave up, and that closing
        // pitch is degree one again — music21 asks the whole realization
        // which of its pitches stand on the degrees wanted, so the first
        // degree of A minor answers with both `A3` and `A4`.
        let count = octave.len().saturating_sub(1).max(1);
        Ok(octave
            .into_iter()
            .enumerate()
            .filter(|(index, _)| degrees.contains(&(index % count + 1)))
            .map(|(_, pitch)| pitch)
            .collect())
    }

    /// Every pitch of the named degrees between two pitches: music21's
    /// `pitchesFromScaleDegrees` given a range, so the third and seventh of
    /// C major from `c2` to `c6` are `D2 G2 D3 G3 D4 G4 D5 G5`.
    pub fn pitches_from_scale_degrees_between(
        &self,
        degrees: &[usize],
        minimum: &Pitch,
        maximum: &Pitch,
    ) -> Result<Vec<Pitch>> {
        let wanted: Vec<String> = self
            .pitches_from_scale_degrees(degrees)?
            .iter()
            .map(Pitch::name)
            .collect();
        Ok(self
            .pitches_between(minimum, maximum)?
            .into_iter()
            .filter(|pitch| wanted.contains(&pitch.name()))
            .collect())
    }

    /// Returns the interval from one degree to another, both folded into the
    /// first octave the way music21's `pitchFromDegree` folds them, so degree
    /// 9 of a seven-note scale is degree 2 and the interval from 2 to 9 is a
    /// unison.
    pub fn interval_between_degrees(&self, start: usize, end: usize) -> Result<Interval> {
        Interval::between_pitches(
            &self.pitch_at_degree(start as IntegerType)?,
            &self.pitch_at_degree(end as IntegerType)?,
        )
    }

    /// Returns whether `other` is the scale pitch `steps` degrees above
    /// `origin`, compared by name so the octave does not matter: music21's
    /// `isNext`.
    pub fn is_next(&self, other: &Pitch, origin: &Pitch, steps: usize) -> Result<bool> {
        Ok(self.next_pitch_above(origin, steps)?.name() == other.name())
    }

    /// Splits pitches into those whose names the scale contains and those it
    /// does not: music21's `match`. The matched list carries the scale's own
    /// pitches, realized from the tonic in octave 4 when it has none, and
    /// the unmatched list carries the pitches as given.
    pub fn match_pitches(&self, pitches: &[Pitch]) -> Result<(Vec<Pitch>, Vec<Pitch>)> {
        self.match_pitches_by(pitches, DegreeComparison::Name)
    }

    /// The same, saying how a pitch is matched against a degree.
    ///
    /// Both lists hold the pitches as given rather than the scale's own —
    /// music21 hands its targets straight back — except that one with no
    /// octave is heard in octave 4, since that is where the scale sounds.
    pub fn match_pitches_by(
        &self,
        pitches: &[Pitch],
        comparison: DegreeComparison,
    ) -> Result<(Vec<Pitch>, Vec<Pitch>)> {
        let realized = self.realized_in_implicit_octave()?;
        let degrees: Vec<String> = realized.iter().map(|p| comparison.key(p)).collect();
        let mut matched = Vec::new();
        let mut unmatched = Vec::new();
        for pitch in pitches {
            let mut heard = pitch.clone();
            if heard.octave().is_none() {
                heard.octave_setter(Some(crate::defaults::PITCH_OCTAVE as IntegerType));
            }
            if degrees.contains(&comparison.key(&heard)) {
                matched.push(heard);
            } else {
                unmatched.push(heard);
            }
        }
        Ok((matched, unmatched))
    }

    /// Returns the scale pitches, tonic through octave, whose pitch classes
    /// none of `pitches` has: music21's `findMissing`, so C major against
    /// `C E G` is `D4 F4 A4 B4`.
    pub fn find_missing(&self, pitches: &[Pitch]) -> Result<Vec<Pitch>> {
        let present: Vec<u8> = pitches.iter().map(root::pitch_class).collect();
        Ok(self
            .realized_in_implicit_octave()?
            .into_iter()
            .filter(|candidate| !present.contains(&root::pitch_class(candidate)))
            .collect())
    }

    /// Returns the solfège syllable for a pitch, `do` through `ti` with the
    /// chromatic inflections (`di`, `ra`, …): music21's `solfeg`. Without
    /// `chromatic` the plain syllable of the degree is returned whatever the
    /// accidental. Errors for degrees past seven and alterations past a
    /// double sharp or flat.
    pub fn solfeg(&self, pitch: &Pitch, variant: SolfegVariant, chromatic: bool) -> Result<String> {
        let (degree, accidental) = self.degree_and_accidental_of(pitch)?;
        if degree > 7 {
            return Err(crate::error::Error::Scale(
                "Cannot call solfeg on non-7-degree scales".to_string(),
            ));
        }
        let table = match variant {
            SolfegVariant::Music21 => &SOLFEG_SYLLABLES,
            SolfegVariant::Humdrum => &HUMDRUM_SOLFEG_SYLLABLES,
        };
        let alter = if chromatic {
            accidental.map_or(0, |accidental| accidental.alter() as IntegerType)
        } else {
            0
        };
        let column = usize::try_from(alter + 2)
            .ok()
            .filter(|column| *column < 5)
            .ok_or_else(|| {
                crate::error::Error::Scale(format!(
                    "no solfeg syllable for an alteration of {alter}"
                ))
            })?;
        Ok(table[degree - 1][column].to_string())
    }

    fn realized_in_implicit_octave(&self) -> Result<Vec<Pitch>> {
        self.pitches()
    }

    /// Returns the one-based degree matching a pitch under the given
    /// comparison, or `None` when the scale does not have it.
    pub fn degree_of_by(
        &self,
        pitch: &Pitch,
        comparison: DegreeComparison,
    ) -> Result<Option<usize>> {
        if !self.repeats_at_the_octave() {
            return Ok(self
                .cycle_places_of(pitch, comparison)?
                .first()
                .map(|place| self.degree_at_position(*place)));
        }
        let wanted = comparison.key(pitch);
        Ok(self
            .scale_pitches()?
            .iter()
            .position(|candidate| comparison.key(candidate) == wanted)
            .map(|index| self.degree_at_position(index)))
    }

    /// Every one-based degree the pitch stands on.
    ///
    /// A scale may name the same note twice — Rag Marwa's A is both its
    /// fifth degree and its seventh, since the pattern dips before it closes
    /// — and music21 chooses between them at random. The choosing is left to
    /// the caller; this says what there is to choose from.
    pub fn degrees_of_by(&self, pitch: &Pitch, comparison: DegreeComparison) -> Result<Vec<usize>> {
        if !self.repeats_at_the_octave() {
            return Ok(self
                .cycle_places_of(pitch, comparison)?
                .into_iter()
                .map(|place| self.degree_at_position(place))
                .collect());
        }
        let wanted = comparison.key(pitch);
        Ok(self
            .scale_pitches()?
            .iter()
            .enumerate()
            .filter(|(_, candidate)| comparison.key(candidate) == wanted)
            .map(|(index, _)| self.degree_at_position(index))
            .collect())
    }

    /// Returns the one-based degree whose pitch name matches, ignoring octave,
    /// or `None` when the pitch is not in the scale.
    pub fn degree_of(&self, pitch: &Pitch) -> Result<Option<usize>> {
        if !self.repeats_at_the_octave() {
            return self.degree_of_by(pitch, DegreeComparison::Name);
        }
        let name = pitch.name();
        Ok(self
            .scale_pitches()?
            .iter()
            .position(|candidate| candidate.name() == name)
            .map(|index| self.degree_at_position(index)))
    }

    /// Returns the one-based degree whose pitch class matches, so `F-` finds
    /// the `E` of C major.
    pub fn degree_of_pitch_class(&self, pitch: &Pitch) -> Result<Option<usize>> {
        if !self.repeats_at_the_octave() {
            return self.degree_of_by(pitch, DegreeComparison::PitchClass);
        }
        let pitch_class = pitch.pitch_class().number();
        Ok(self
            .scale_pitches()?
            .iter()
            .position(|candidate| candidate.pitch_class().number() == pitch_class)
            .map(|index| self.degree_at_position(index)))
    }

    /// Returns the scale pitch `steps` degrees above `origin`. A pitch outside
    /// the scale first moves to the nearest scale pitch above it.
    pub fn next_pitch_above(&self, origin: &Pitch, steps: usize) -> Result<Pitch> {
        self.pitch_steps_from(origin, steps as IntegerType, None)
    }

    /// The same, told which side of a pitch outside the scale to start from:
    /// music21's `getNeighbor`, where naming a side means stepping the whole
    /// way from the neighbour on it rather than counting the move onto the
    /// scale as one of the steps.
    pub fn next_pitch_beside(
        &self,
        origin: &Pitch,
        steps: IntegerType,
        below: bool,
    ) -> Result<Pitch> {
        self.pitch_steps_from(origin, steps, Some(below))
    }

    /// Returns the scale pitch `steps` degrees below `origin`. A pitch outside
    /// the scale first moves to the nearest scale pitch below it.
    pub fn next_pitch_below(&self, origin: &Pitch, steps: usize) -> Result<Pitch> {
        self.pitch_steps_from(origin, -(steps as IntegerType), None)
    }

    /// Returns the degree a pitch sits on together with the accidental that
    /// separates it from the scale's own spelling of that degree, so `E-` in
    /// C major is degree three with a flat. Errors when no degree shares the
    /// pitch's letter.
    pub fn degree_and_accidental_of(
        &self,
        pitch: &Pitch,
    ) -> Result<(usize, Option<crate::pitch::Accidental>)> {
        if let Some(degree) = self.degree_of(pitch)? {
            return Ok((degree, None));
        }
        let pitches = self.scale_pitches()?;
        let index = pitches
            .iter()
            .position(|candidate| candidate.step() == pitch.step())
            .ok_or_else(|| {
                crate::error::Error::Scale(format!(
                    "cannot get any scale degree for {pitch} in {self:?}"
                ))
            })?;
        let difference =
            pitch.accidental_or_natural().alter() - pitches[index].accidental_or_natural().alter();
        let accidental = if difference == 0.0 {
            None
        } else {
            Some(crate::pitch::Accidental::new(difference)?)
        };
        Ok((index + 1, accidental))
    }

    fn scale_pitches(&self) -> Result<Vec<Pitch>> {
        let mut pitches = self.pitches()?;
        pitches.truncate(self.degree_count());
        Ok(pitches)
    }

    fn pitch_steps_from(
        &self,
        origin: &Pitch,
        steps: IntegerType,
        neighbour_below: Option<bool>,
    ) -> Result<Pitch> {
        self.pitch_steps_from_place(origin, steps, neighbour_below, 0)
    }

    /// How many places in this scale's realization stand on `origin`.
    ///
    /// A scale may name the same note twice: Rag Marwa's `D-` is both the
    /// note above its tonic and the one it passes through coming down from
    /// the octave, and where the next note is depends on which of them is
    /// meant. music21 chooses between them at random — the choosing is the
    /// caller's, and [`Self::next_pitch_below_from`] takes it.
    pub fn places_of(&self, origin: &Pitch) -> Result<usize> {
        Ok(self.places_on(origin)?.len())
    }

    /// The places of the realization, and the octave shift each stands at,
    /// that sound `origin`.
    fn places_on(&self, origin: &Pitch) -> Result<Vec<(usize, IntegerType)>> {
        if !self.repeats_at_the_octave() {
            return Ok(self
                .cycle_place_sounding(origin, DegreeComparison::Name)?
                .map(|(_, place)| (place, 0))
                .into_iter()
                .collect());
        }
        let pitches = self.scale_pitches()?;
        let origin_ps = origin.ps();
        let name = origin.name();
        let base_shift = ((origin_ps - self.tonic.ps()) / 12.0).floor() as IntegerType;
        Ok((base_shift - 1..=base_shift + 1)
            .flat_map(|shift| {
                pitches.iter().enumerate().map(move |(index, pitch)| {
                    (pitch.ps() + 12.0 * shift as FloatType, index, shift)
                })
            })
            .filter(|(ps, index, _)| {
                pitches[*index].name() == name && (ps - origin_ps).abs() < 1e-9
            })
            .map(|(_, index, shift)| (index, shift))
            .collect())
    }

    /// The note `steps` below `origin`, read as the `place`-th of the places
    /// this scale stands that note on. See [`Self::places_of`].
    pub fn next_pitch_below_from(
        &self,
        origin: &Pitch,
        steps: usize,
        place: usize,
    ) -> Result<Pitch> {
        self.pitch_steps_from_place(origin, -(steps as IntegerType), None, place)
    }

    /// The note `steps` above `origin`, read the same way.
    pub fn next_pitch_above_from(
        &self,
        origin: &Pitch,
        steps: usize,
        place: usize,
    ) -> Result<Pitch> {
        self.pitch_steps_from_place(origin, steps as IntegerType, None, place)
    }

    fn pitch_steps_from_place(
        &self,
        origin: &Pitch,
        steps: IntegerType,
        neighbour_below: Option<bool>,
        place: usize,
    ) -> Result<Pitch> {
        if steps == 0 {
            return Err(crate::error::Error::Scale(
                "step size must be at least 1".to_string(),
            ));
        }
        if !self.repeats_at_the_octave() {
            return self.cycle_steps_from(origin, steps, neighbour_below);
        }
        let pitches = self.scale_pitches()?;
        let count = pitches.len() as IntegerType;
        let origin_ps = origin.ps();
        let base_shift = ((origin_ps - self.tonic.ps()) / 12.0).floor() as IntegerType;
        let candidates = (base_shift - 1..=base_shift + 1)
            .flat_map(|shift| {
                pitches.iter().enumerate().map(move |(index, pitch)| {
                    (pitch.ps() + 12.0 * shift as FloatType, index, shift)
                })
            })
            .collect::<Vec<_>>();
        let ascending = steps > 0;
        let standing = self.places_on(origin)?;
        let (index, shift, remaining) = match standing.get(place % standing.len().max(1)).copied() {
            Some((index, shift)) => (index, shift, steps),
            None => {
                // Which side of the pitch to come onto the scale at: the way
                // the move is going unless a caller has said otherwise, and
                // then the move onto the scale counts as one of the steps.
                let take_below = neighbour_below.unwrap_or(!ascending);
                let neighbour = if take_below {
                    candidates
                        .iter()
                        .filter(|(ps, _, _)| *ps < origin_ps)
                        .max_by(|left, right| left.0.total_cmp(&right.0))
                } else {
                    candidates
                        .iter()
                        .filter(|(ps, _, _)| *ps > origin_ps)
                        .min_by(|left, right| left.0.total_cmp(&right.0))
                };
                let &(_, index, shift) = neighbour.ok_or_else(|| {
                    crate::error::Error::Scale(format!("no scale pitch beside {origin}"))
                })?;
                let remaining = if neighbour_below.is_some() {
                    steps
                } else {
                    steps - steps.signum()
                };
                (index, shift, remaining)
            }
        };
        let total = index as IntegerType + remaining;
        let mut pitch = pitches[total.rem_euclid(count) as usize].clone();
        let octave_shift = shift + total.div_euclid(count);
        let octave = origin.octave().map(|_| {
            pitch
                .octave()
                .unwrap_or(crate::defaults::PITCH_OCTAVE as IntegerType)
                + octave_shift
        });
        pitch.octave_setter(octave);
        Ok(pitch)
    }
}

/// Whether a note sounds inside a range, allowing it the hundred-thousandth
/// of a semitone music21 allows: a scale read from a file in cents lands its
/// octave a hair past where the range ends, and still closes on it.
fn within(sounding: FloatType, lowest: FloatType, highest: FloatType) -> bool {
    sounding > lowest - SLACK && sounding < highest + SLACK
}

/// The slack [`within`] allows at either end of a range, in semitones.
const SLACK: FloatType = 1e-5;

/// A collection's notes with the octaves a caller left out filled in so that
/// the collection rises: music21's `fixDefaultOctaveForPitchList`.
///
/// `A B C D E F G# A` is a scale on A, not a collection that climbs a tone
/// and then falls a seventh, and a caller who names no octaves means the
/// first. Notes that carry an octave are left exactly as they are.
pub(super) fn rising_octaves(pitches: &[Pitch]) -> Vec<Pitch> {
    let mut risen: Vec<Pitch> = Vec::with_capacity(pitches.len());
    let mut last_ps = 0.0;
    let mut last_octave =
        pitches
            .first()
            .map_or(crate::defaults::PITCH_OCTAVE as IntegerType, |pitch| {
                pitch
                    .octave()
                    .unwrap_or(crate::defaults::PITCH_OCTAVE as IntegerType)
            });
    for pitch in pitches {
        let mut pitch = pitch.clone();
        if pitch.octave().is_none() {
            if last_ps > pitch.ps() {
                pitch.octave_setter(Some(last_octave));
            }
            while last_ps > pitch.ps() {
                last_octave += 1;
                pitch.octave_setter(Some(last_octave));
            }
        }
        last_ps = pitch.ps();
        risen.push(pitch);
    }
    risen
}
