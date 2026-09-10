//! The notation a chord carries through its notes: volume, colour,
//! notehead, stem, beams, tie and lyrics, and the intervals a chord can
//! be annotated with.

use super::*;

impl Chord {
    /// Whether *every* note carries a volume of its own: music21's
    /// `hasComponentVolumes`, which counts the notes that have one and
    /// compares the count against the whole chord. A chord where only some
    /// notes have been given a volume reads as having none.
    pub fn has_component_volumes(&self) -> bool {
        self.notes.iter().all(Note::has_volume_information)
    }

    /// How loud the chord is. With volumes on its notes this is their
    /// average velocity, as music21 reads it; otherwise it is the chord's
    /// own volume.
    pub fn volume(&self) -> Volume {
        // music21 asks in this order: a volume of the chord's own wins, then
        // the notes' average, and a chord with neither — an empty one
        // included — reads as a default volume.
        if let Some(volume) = &self.volume {
            return volume.clone();
        }
        if !self.has_component_volumes() {
            return Volume::default();
        }
        let velocities: Vec<IntegerType> = self
            .notes
            .iter()
            .filter_map(|note| note.volume().velocity())
            .collect();
        if velocities.is_empty() {
            return Volume::default();
        }
        let total: IntegerType = velocities.iter().sum();
        let mean = FloatType::from(total) / velocities.len() as FloatType;
        Volume::from_velocity(mean.round_ties_even() as IntegerType)
    }

    /// Sets the chord's own volume, which drops any the notes carried.
    pub fn set_volume(&mut self, volume: Option<Volume>) {
        for note in &mut self.notes {
            note.set_volume(None);
        }
        self.volume = volume;
    }

    /// Gives each note a volume from the list, cycling through it when the
    /// chord has more notes than the list has volumes: music21's
    /// `setVolumes`. The chord's own volume is dropped.
    pub fn set_volumes(&mut self, volumes: &[Volume]) -> Result<()> {
        if volumes.is_empty() {
            return Err(Error::Chord(
                "setVolumes needs at least one volume".to_string(),
            ));
        }
        self.volume = None;
        for (index, note) in self.notes.iter_mut().enumerate() {
            note.set_volume(Some(volumes[index % volumes.len()].clone()));
        }
        Ok(())
    }

    /// The colour the chord is written in, when the chord itself carries one
    /// rather than its notes.
    pub fn color(&self) -> Option<&str> {
        self.color.as_deref()
    }

    /// Sets the colour the chord as a whole is written in.
    pub fn set_color(&mut self, color: Option<String>) {
        self.color = color;
    }

    /// The shape the chord as a whole is drawn with: music21's `notehead`,
    /// which a chord has in its own right and not only through its notes.
    pub fn notehead(&self) -> Notehead {
        self.notehead
    }

    /// Sets that shape.
    pub fn set_notehead(&mut self, notehead: Notehead) {
        self.notehead = notehead;
    }

    /// Whether the chord's own note heads are filled, when it says.
    pub fn notehead_fill(&self) -> Option<bool> {
        self.notehead_fill
    }

    /// Says whether they are filled.
    pub fn set_notehead_fill(&mut self, fill: Option<bool>) {
        self.notehead_fill = fill;
    }

    /// Whether the chord's own note heads are bracketed.
    pub fn notehead_parenthesis(&self) -> bool {
        self.notehead_parenthesis
    }

    /// Says whether they are bracketed.
    pub fn set_notehead_parenthesis(&mut self, parenthesis: bool) {
        self.notehead_parenthesis = parenthesis;
    }

    /// Which way the chord's own stem points.
    pub fn stem_direction(&self) -> StemDirection {
        self.stem_direction
    }

    /// Sets which way it points.
    pub fn set_stem_direction(&mut self, direction: StemDirection) {
        self.stem_direction = direction;
    }

    /// The beams joining the chord's flags to its neighbours'.
    pub fn beams(&self) -> &Beams {
        &self.beams
    }

    /// Replaces those beams.
    pub fn set_beams(&mut self, beams: Beams) {
        self.beams = beams;
    }

    /// The colour a pitch is written in: the note's own colour when it has
    /// one, and the chord's otherwise, as music21's `getColor` reads it.
    pub fn color_of_pitch(&self, pitch: &Pitch) -> Option<&str> {
        self.note_for_pitch(pitch)
            .and_then(Note::color)
            .or(self.color())
    }

    /// The tie of the first note that carries one: music21's chord-level
    /// `tie`.
    pub fn tie(&self) -> Option<&Tie> {
        self.notes.iter().find_map(Note::tie)
    }

    /// Ties every note in the chord, or unties them all with `None`.
    pub fn set_tie(&mut self, tie: Option<Tie>) {
        for note in &mut self.notes {
            note.set_tie(tie.clone());
        }
    }

    /// The syllables sung on the chord, kept on its first note as music21
    /// keeps them on the chord itself.
    pub fn lyrics(&self) -> &[Lyric] {
        self.notes.first().map_or(&[], Note::lyrics)
    }

    /// Adds a syllable as the next verse: music21's `addLyric`.
    pub fn add_lyric(
        &mut self,
        text: &str,
        number: Option<IntegerType>,
        apply_raw: bool,
    ) -> Result<()> {
        match self.notes.first_mut() {
            Some(note) => note.add_lyric(text, number, apply_raw),
            None => Err(Error::Chord(
                "an empty chord has nothing to sing".to_string(),
            )),
        }
    }

    /// Names the interval from the chord's lowest pitch up to each of the
    /// others, highest first: music21's `annotateIntervals`.
    ///
    /// With `strip_specifiers` the names are bare numbers (`8`, `5`, `3`)
    /// and sorted downward; without it they are full interval names (`P8`,
    /// `P5`, `M3`). Repeated pitches are dropped first, and `sort_pitches`
    /// measures from the lowest pitch rather than the written first one.
    pub fn annotate_intervals(
        &self,
        strip_specifiers: bool,
        sort_pitches: bool,
    ) -> Result<Vec<String>> {
        let mut reduced = self.remove_redundant_pitches();
        if sort_pitches {
            reduced = reduced.sort_ascending();
        }
        let pitches = reduced.pitches();
        let Some(lowest) = pitches.first() else {
            return Ok(Vec::new());
        };
        let mut names = Vec::with_capacity(pitches.len().saturating_sub(1));
        for pitch in pitches.iter().skip(1).rev() {
            let interval = Interval::between_pitches(lowest, pitch)?;
            names.push(if strip_specifiers {
                interval.generic().semi_simple_undirected().to_string()
            } else {
                interval.semi_simple_name()
            });
        }
        if strip_specifiers && sort_pitches {
            names.sort_by(|left, right| right.cmp(left));
        }
        Ok(names)
    }

    /// Writes the interval names of [`Self::annotate_intervals`] onto the
    /// chord as lyrics, one verse each.
    pub fn annotated_with_intervals(
        &self,
        strip_specifiers: bool,
        sort_pitches: bool,
    ) -> Result<Self> {
        let names = self.annotate_intervals(strip_specifiers, sort_pitches)?;
        let mut annotated = self.clone();
        for name in names {
            annotated.add_lyric(&name, None, false)?;
        }
        Ok(annotated)
    }

    /// Reads a string harmonic: given a chord whose second note is written
    /// with a diamond head, the sounding pitch is the harmonic of the first
    /// note that their distance picks out, and the chord comes back with it
    /// added on top. A chord not written as a harmonic answers `None`.
    ///
    /// This is music21's `Pitch.getStringHarmonic`, which reads the notehead
    /// off the chord rather than off the pitch it is called on.
    pub fn string_harmonic(&self) -> Result<Option<Self>> {
        let [stopped, touched] = match self.notes.get(..2) {
            Some([first, second]) => [first, second],
            _ => return Ok(None),
        };
        if touched.notehead() != Notehead::Diamond {
            return Ok(None);
        }
        let distance = crate::interval::notes_to_chromatic(&stopped.pitch, &touched.pitch);
        let harmonic = match distance.interval_class() {
            0 => 2,
            7 => 3,
            5 => 4,
            4 => 5,
            3 => 6,
            6 => 7,
            _ => 1,
        };
        let sounding = if harmonic == 1 {
            stopped.pitch.clone()
        } else {
            stopped.pitch.harmonic(harmonic)?
        };
        let mut sounding_note = Note::from_pitch(sounding);
        sounding_note.set_notehead_parenthesis(true);
        sounding_note.set_notehead_fill(Some(true));
        sounding_note.set_stem_direction(crate::notation::StemDirection::NoStem);
        let mut touched_note = Note::from_pitch(touched.pitch.clone());
        touched_note.set_notehead(touched.notehead());
        let notes = vec![
            Note::from_pitch(stopped.pitch.clone()),
            touched_note,
            sounding_note,
        ];
        Ok(Some(Self::new(notes.as_slice())?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notation::TieType;

    #[test]
    fn annotate_intervals_matches_music21() {
        let triad = Chord::new("C4 E4 G4").unwrap();
        assert_eq!(triad.annotate_intervals(true, true).unwrap(), ["5", "3"]);
        assert_eq!(triad.annotate_intervals(false, true).unwrap(), ["P5", "M3"]);
        let with_octave = Chord::new("C4 E4 G4 C5").unwrap();
        assert_eq!(
            with_octave.annotate_intervals(true, true).unwrap(),
            ["8", "5", "3"]
        );
        assert_eq!(
            with_octave.annotate_intervals(false, true).unwrap(),
            ["P8", "P5", "M3"]
        );
        // A pitch repeated at the same octave drops out before the
        // intervals are read; one an octave up is a tenth and stays, which
        // reads as another third.
        let doubled = Chord::new("C4 E4 G4 E4").unwrap();
        assert_eq!(doubled.annotate_intervals(true, true).unwrap(), ["5", "3"]);
        let spread = Chord::new("C4 E4 G4 E5").unwrap();
        assert_eq!(
            spread.annotate_intervals(true, true).unwrap(),
            ["5", "3", "3"]
        );
        assert!(
            Chord::empty()
                .annotate_intervals(true, true)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn annotated_intervals_become_lyrics() {
        let annotated = Chord::new("C4 E4 G4")
            .unwrap()
            .annotated_with_intervals(true, true)
            .unwrap();
        let texts: Vec<String> = annotated.lyrics().iter().map(Lyric::text).collect();
        assert_eq!(texts, ["5", "3"]);
        assert_eq!(annotated.lyrics()[1].number(), 2);
    }

    #[test]
    fn component_volumes_average_into_the_chord() {
        let mut chord = Chord::new("C4 E4 G4").unwrap();
        assert!(!chord.has_component_volumes());
        assert_eq!(chord.volume().velocity(), None);

        chord
            .set_volumes(&[
                Volume::from_velocity(60),
                Volume::from_velocity(20),
                Volume::from_velocity(120),
            ])
            .unwrap();
        assert!(chord.has_component_volumes());
        let velocities: Vec<Option<IntegerType>> = chord
            .notes()
            .iter()
            .map(|note| note.volume().velocity())
            .collect();
        assert_eq!(velocities, [Some(60), Some(20), Some(120)]);
        assert_eq!(chord.volume().velocity(), Some(67));

        // A volume set on the chord replaces the ones on its notes.
        chord.set_volume(Some(Volume::from_velocity(90)));
        assert!(!chord.has_component_volumes());
        assert_eq!(chord.volume().velocity(), Some(90));
        assert!(chord.set_volumes(&[]).is_err());
    }

    #[test]
    fn a_shorter_volume_list_cycles() {
        let mut chord = Chord::new("C4 E4 G4 B-4").unwrap();
        chord
            .set_volumes(&[Volume::from_velocity(40), Volume::from_velocity(80)])
            .unwrap();
        let velocities: Vec<Option<IntegerType>> = chord
            .notes()
            .iter()
            .map(|note| note.volume().velocity())
            .collect();
        assert_eq!(velocities, [Some(40), Some(80), Some(40), Some(80)]);
    }

    #[test]
    fn ties_and_colours_reach_the_notes() {
        let mut chord = Chord::new("C4 E4 G4").unwrap();
        assert!(chord.tie().is_none());
        chord.set_tie(Some(Tie::new(TieType::Start)));
        assert_eq!(chord.tie().map(Tie::tie_type), Some(TieType::Start));
        assert!(chord.notes().iter().all(|note| note.tie().is_some()));
        chord.set_tie(None);
        assert!(chord.tie().is_none());

        let e4 = Pitch::from_name("E4").unwrap();
        chord.set_color(Some("blue".to_string()));
        assert_eq!(chord.color_of_pitch(&e4), Some("blue"));
        chord
            .note_for_pitch_mut(&e4)
            .unwrap()
            .set_color(Some("red".to_string()));
        assert_eq!(chord.color_of_pitch(&e4), Some("red"));
        let c4 = Pitch::from_name("C4").unwrap();
        assert_eq!(chord.color_of_pitch(&c4), Some("blue"));
    }

    #[test]
    fn a_diamond_notehead_reads_as_a_string_harmonic() {
        let mut chord = Chord::new("D3 G3").unwrap();
        assert!(chord.string_harmonic().unwrap().is_none());
        chord.notes_mut()[1].set_notehead(Notehead::Diamond);
        let sounded = chord.string_harmonic().unwrap().unwrap();
        assert_eq!(
            sounded
                .pitches()
                .iter()
                .map(Pitch::name_with_octave)
                .collect::<Vec<_>>(),
            ["D3", "G3", "D5"]
        );
        let sounding = &sounded.notes()[2];
        assert!(sounding.notehead_parenthesis());
        assert_eq!(sounding.stem_direction(), StemDirection::NoStem);
        assert_eq!(sounded.notes()[1].notehead(), Notehead::Diamond);
    }
}
