//! Sounding a numeral: the figured-bass column read over the key's
//! scale from the degree the inversion names, then respelled to the
//! quality the numeral asks for.

use super::*;

impl RomanNumeral {
    /// The collection the figure's degrees are read off.
    pub(super) fn reading(&self) -> Result<Reading> {
        if let Some(scale) = &self.scale {
            return Ok(Reading::Scale(scale.clone()));
        }
        let key = self.effective_key()?;
        // Every augmented sixth is read in the parallel minor, whatever key
        // it is written in: its sixth degree is the flat one.
        if matches!(self.kind, RomanKind::AugmentedSixth(_)) && key.mode() != "minor" {
            return Ok(Reading::Key(Key::from_tonic_mode(
                &key.tonic_pitch().name(),
                Some("minor"),
            )?));
        }
        Ok(Reading::Key(key))
    }

    /// The chord the numeral stands for, spelled where its key sounds.
    ///
    /// This is music21's `_updatePitches`, and it is a figured-bass reading
    /// rather than a stack of intervals: the bass is the scale degree the
    /// inversion figure puts there, every number of the column is that many
    /// scale steps above it, and only then is the result respelled to the
    /// quality the numeral's case and symbols asked for. Reading it off the
    /// scale is what lets a numeral mean something in a mode, and what makes
    /// `V7b5` alter one note rather than name a different chord.
    pub fn to_chord(&self) -> Result<Chord> {
        let reading = self.reading()?;
        let numbers = self.figures.numbers();
        let implies_root = FIGURES_IMPLYING_ROOT.contains(&numbers.as_slice());
        let bass_degree = self.bass_scale_degree(&numbers, implies_root)?;

        let mut pitches = vec![reading.pitch_at(bass_degree)?];
        for figure in self.figures.figures().iter().rev() {
            let Some(number) = figure.number() else {
                continue;
            };
            let degree = bass_degree + number - 1;
            let mut pitch = figure.modifier().modify(&reading.pitch_at(degree)?)?;
            let below = pitches.last().map_or(0.0, Pitch::ps);
            if pitch.ps() < below {
                pitch.set_octave(Some(pitch.octave().unwrap_or(4) + 1));
            }
            pitches.push(pitch);
        }

        // The alteration in front of the numeral moves the chord without
        // renaming it — music21 transposes by an augmented unison — but it
        // leaves the notes above the fifth alone, since a `bVII9` is a chord
        // on a flattened seventh degree and not a flattened ninth.
        if self.accidental != 0 {
            let untouched = upper_extension_indices(&pitches)?;
            for (index, pitch) in pitches.iter_mut().enumerate() {
                if untouched.contains(&index) {
                    continue;
                }
                let alter = pitch.accidental().alter() + FloatType::from(self.accidental);
                pitch.set_accidental(Some(Accidental::new(alter)?));
            }
        }

        // A column that says nothing about a root is a stack over its bass.
        let root = if implies_root {
            None
        } else {
            Some(pitches[0].clone())
        };

        self.match_accidentals_to_quality(&mut pitches, root.as_ref())?;
        self.correct_bracketed_pitches(&mut pitches, root.as_ref())?;

        // A note left out or put in must not move the root, so the root is
        // read while the chord is still whole and recorded from there.
        let altered = !self.figures.omitted.is_empty() || !self.figures.added.is_empty();
        let recorded = match &root {
            Some(root) => Some(root.clone()),
            None if altered => Chord::new(pitches.as_slice())?.root().cloned(),
            None => None,
        };

        self.omit_steps(&mut pitches, recorded.as_ref())?;
        self.add_steps(&mut pitches, &reading)?;

        let mut chord = Chord::new(pitches.as_slice())?;
        chord.set_root(recorded);
        Ok(chord)
    }

    /// The scale degree the inversion figure puts in the bass.
    pub(super) fn bass_scale_degree(
        &self,
        numbers: &[u8],
        implies_root: bool,
    ) -> Result<IntegerType> {
        if !implies_root {
            return Ok(IntegerType::from(self.degree));
        }
        bass_scale_degree_from_notation_in(self.degree, numbers, self.reading()?.cardinality())
            .map(IntegerType::from)
    }

    /// music21's `_matchAccidentalsToQuality`, over the chord being built:
    /// an accidental written on a figure is left where it was put, which is
    /// what keeps the flat of `V7b5`.
    pub(super) fn match_accidentals_to_quality(
        &self,
        pitches: &mut [Pitch],
        root: Option<&Pitch>,
    ) -> Result<()> {
        let written: Vec<u8> = [3u8, 5, 7]
            .into_iter()
            .filter(|step| self.figures.alters(*step))
            .collect();
        match_pitches_to_quality(pitches, root, self.implied_quality, &written)
    }

    /// music21's `_correctBracketedPitches`: an alteration written in square
    /// brackets moves a chord step without changing which step it is.
    pub(super) fn correct_bracketed_pitches(
        &self,
        pitches: &mut [Pitch],
        root: Option<&Pitch>,
    ) -> Result<()> {
        for (alter, step) in &self.figures.bracketed {
            let Some(index) = chord_step_index(pitches, root, *step)? else {
                continue;
            };
            let moved = pitches[index].accidental().alter() + FloatType::from(*alter);
            pitches[index].set_accidental(Some(Accidental::new(moved)?));
        }
        Ok(())
    }

    /// music21's omitted steps: a `[no3]` drops every note of that step.
    pub(super) fn omit_steps(&self, pitches: &mut Vec<Pitch>, root: Option<&Pitch>) -> Result<()> {
        if self.figures.omitted.is_empty() {
            return Ok(());
        }
        let mut dropped = Vec::new();
        for step in &self.figures.omitted {
            if let Some(index) = chord_step_index(pitches, root, *step)? {
                dropped.push(pitches[index].name());
            }
        }
        pitches.retain(|pitch| !dropped.contains(&pitch.name()));
        Ok(())
    }

    /// music21's added steps: an `[add4]` puts in the note that many scale
    /// steps above the *root*, at or above the bass.
    pub(super) fn add_steps(&self, pitches: &mut Vec<Pitch>, reading: &Reading) -> Result<()> {
        if self.figures.added.is_empty() {
            return Ok(());
        }
        let bass = pitches.first().map_or(0.0, Pitch::ps);
        for (alter, step) in &self.figures.added {
            let degree = IntegerType::from(self.degree) + IntegerType::from(*step) - 1;
            let mut added = reading.pitch_at(degree)?;
            let moved = added.accidental().alter() + FloatType::from(*alter);
            added.set_accidental(Some(Accidental::new(moved)?));
            while added.ps() < bass {
                added.set_octave(Some(added.octave().unwrap_or(4) + 1));
            }
            // An added note spelled onto the bass belongs above it, not under
            // it: `IV[add#7]` in C would otherwise put `E#` in the bass.
            if added.ps() == bass
                && pitches
                    .first()
                    .is_some_and(|low| added.diatonic_note_number() < low.diatonic_note_number())
            {
                added.set_octave(Some(added.octave().unwrap_or(4) + 1));
            }
            if !pitches
                .iter()
                .any(|pitch| pitch.name_with_octave() == added.name_with_octave())
            {
                pitches.push(added);
            }
        }
        // Two notes may sound alike and still be written apart, and the one
        // written lower is the one written first.
        pitches.sort_by(|left, right| {
            left.ps()
                .partial_cmp(&right.ps())
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(
                    left.diatonic_note_number()
                        .cmp(&right.diatonic_note_number()),
                )
        });
        Ok(())
    }
}

/// Respells the third, fifth and seventh of a chord to the quality asked for.
///
/// This is music21's `_matchAccidentalsToQuality`. The letters come from
/// wherever the notes came from — a scale, usually — and the quality decides
/// only the accidentals, so a minor reading of `C E G` gives `C E- G` and
/// keeps the letters it was handed. Chord steps listed in `written` are left
/// alone, which is how an accidental somebody wrote survives the correction.
pub fn match_pitches_to_quality(
    pitches: &mut [Pitch],
    root: Option<&Pitch>,
    quality: ImpliedQuality,
    written: &[u8],
) -> Result<()> {
    let correct = quality.correct_semitones();
    for (step, want) in [3u8, 5, 7].into_iter().zip(correct.iter().copied()) {
        if written.contains(&step) {
            continue;
        }
        let Some(index) = chord_step_index(pitches, root, step)? else {
            continue;
        };
        let have = step_semitones(pitches, root, index)?;
        if have == IntegerType::from(want) {
            continue;
        }
        correct_faulty_pitch(&mut pitches[index], IntegerType::from(want) - have)?;
    }

    // A seventh does not have to match the scale: an `i7` read in a major key
    // would otherwise take the major seventh the scale spells.
    if correct.len() == 2
        && quality == ImpliedQuality::Minor
        && !written.contains(&7)
        && let Some(index) = chord_step_index(pitches, root, 7)?
        && step_semitones(pitches, root, index)? == 11
    {
        correct_faulty_pitch(&mut pitches[index], -1)?;
    }
    Ok(())
}

/// What a roman numeral counts its degrees against.
///
/// Usually a key, which has seven of them; but music21 reads a numeral over
/// any concrete scale, and an octatonic one has eight.
pub(super) enum Reading {
    Key(Key),
    Scale(crate::scale::Scale),
}

impl Reading {
    /// How many degrees there are before the collection repeats.
    fn cardinality(&self) -> u8 {
        match self {
            Self::Key(_) => 7,
            Self::Scale(scale) => scale.degree_count() as u8,
        }
    }

    /// The pitch a degree spells, folded into the octave the collection's
    /// tonic stands in.
    fn pitch_at(&self, degree: IntegerType) -> Result<Pitch> {
        match self {
            Self::Key(key) => degree_pitch(key, degree),
            Self::Scale(scale) => {
                let count = IntegerType::from(self.cardinality());
                let wrapped = (degree - 1).rem_euclid(count) + 1;
                scale.pitch_at_degree(wrapped)
            }
        }
    }
}

/// The pitch a scale degree spells, folded into the octave the scale's tonic
/// stands in — which is what music21's `pitchFromDegree` does, so a ninth
/// comes back as the second and the caller lifts it.
pub(super) fn degree_pitch(key: &Key, degree: IntegerType) -> Result<Pitch> {
    let wrapped = (degree - 1).rem_euclid(7) + 1;
    key.pitch_from_degree(wrapped as usize)
}

/// The natural note at a diatonic note number, where 22 is middle C.
pub(super) fn natural_at_diatonic_number(number: IntegerType) -> Result<Pitch> {
    const LETTERS: [char; 7] = ['C', 'D', 'E', 'F', 'G', 'A', 'B'];
    let letter = LETTERS[((number - 1).rem_euclid(7)) as usize];
    Pitch::builder()
        .step(letter)
        .octave((number - 1).div_euclid(7))
        .build()
}

/// Which of a chord's notes are the seventh and the extensions above it.
///
/// music21 leaves these alone when it moves a chord by the alteration in
/// front of its numeral, since the alteration is written against the root.
pub(super) fn upper_extension_indices(pitches: &[Pitch]) -> Result<Vec<usize>> {
    let chord = Chord::new(pitches)?;
    let Some(root) = chord.root().cloned() else {
        return Ok(Vec::new());
    };
    let mut indices = Vec::new();
    for step in [7u8, 2, 4, 6] {
        if let Some(index) = chord_step_index(pitches, Some(&root), step)? {
            indices.push(index);
        }
    }
    Ok(indices)
}

/// Where a chord step stands among a set of pitches, counting from the root
/// the chord infers when none was recorded.
pub(super) fn chord_step_index(
    pitches: &[Pitch],
    root: Option<&Pitch>,
    step: u8,
) -> Result<Option<usize>> {
    let inferred;
    let root = match root {
        Some(root) => root,
        None => {
            let chord = Chord::new(pitches)?;
            let Some(found) = chord.root().cloned() else {
                return Ok(None);
            };
            inferred = found;
            &inferred
        }
    };
    let wanted = IntegerType::from(step);
    Ok(pitches.iter().position(|pitch| {
        (pitch.diatonic_note_number() - root.diatonic_note_number()).rem_euclid(7) + 1 == wanted
    }))
}

/// How many semitones a pitch stands above the root, within the octave.
pub(super) fn step_semitones(
    pitches: &[Pitch],
    root: Option<&Pitch>,
    index: usize,
) -> Result<IntegerType> {
    let inferred;
    let root = match root {
        Some(root) => root,
        None => {
            let chord = Chord::new(pitches)?;
            let Some(found) = chord.root().cloned() else {
                return Ok(0);
            };
            inferred = found;
            &inferred
        }
    };
    let distance = (pitches[index].ps() - root.ps()).round() as IntegerType;
    Ok(distance.rem_euclid(12))
}

/// music21's `correctFaultyPitch`: moves a note by the semitones it is out
/// by, reading a correction of half an octave or more the short way round.
pub(super) fn correct_faulty_pitch(pitch: &mut Pitch, correction: IntegerType) -> Result<()> {
    let folded = fold_correction(correction) + pitch.accidental().alter() as IntegerType;
    let alter = fold_correction(folded);
    pitch.set_accidental(Some(Accidental::new(FloatType::from(alter))?));
    Ok(())
}

/// Half an octave or more in either direction is the same note the other way.
pub(super) fn fold_correction(semitones: IntegerType) -> IntegerType {
    if semitones >= 6 {
        semitones - 12
    } else if semitones <= -6 {
        semitones + 12
    } else {
        semitones
    }
}
