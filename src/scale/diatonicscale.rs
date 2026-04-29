use crate::{chord::Chord, defaults::IntegerType, error::Result, pitch::Pitch};

use super::concretescale::ConcreteScale;

#[derive(Clone, Debug)]
pub(crate) struct DiatonicScale {
    concrete: ConcreteScale,
    mode: String,
}

impl DiatonicScale {
    pub(crate) fn new(tonic: Pitch, sharps: IntegerType, mode: &str) -> Self {
        Self {
            concrete: ConcreteScale::new(tonic, sharps),
            mode: mode.to_string(),
        }
    }

    pub(crate) fn mode(&self) -> &str {
        &self.mode
    }

    pub(crate) fn tonic(&self) -> &Pitch {
        self.concrete.tonic()
    }

    pub(crate) fn pitch_from_degree(&self, degree: usize) -> Result<Pitch> {
        self.concrete.pitch_from_degree(degree)
    }

    pub(crate) fn pitches(&self) -> Result<Vec<Pitch>> {
        self.concrete.pitches()
    }

    pub(crate) fn triad_from_degree(&self, degree: usize) -> Result<Chord> {
        let notes = vec![
            self.pitch_from_degree(degree)?,
            self.pitch_from_degree(degree + 2)?,
            self.pitch_from_degree(degree + 4)?,
        ];
        Chord::new(notes.as_slice())
    }

    pub(crate) fn seventh_chord_from_degree(&self, degree: usize) -> Result<Chord> {
        let notes = vec![
            self.pitch_from_degree(degree)?,
            self.pitch_from_degree(degree + 2)?,
            self.pitch_from_degree(degree + 4)?,
            self.pitch_from_degree(degree + 6)?,
        ];
        Chord::new(notes.as_slice())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pitch(name: &str) -> Pitch {
        Pitch::new(
            Some(name.to_string()),
            None,
            None,
            Option::<IntegerType>::None,
            Option::<IntegerType>::None,
            None,
            None,
            None,
            None,
        )
        .expect("valid pitch")
    }

    #[test]
    fn diatonic_scale_degree_lookup() {
        let scale = DiatonicScale::new(pitch("A4"), 0, "minor");
        assert_eq!(scale.pitch_from_degree(1).unwrap().name_with_octave(), "A4");
        assert_eq!(scale.pitch_from_degree(3).unwrap().name_with_octave(), "C5");
        assert_eq!(scale.pitch_from_degree(7).unwrap().name_with_octave(), "G5");
    }

    #[test]
    fn diatonic_scale_degree_chords() {
        let scale = DiatonicScale::new(pitch("C4"), 0, "major");
        assert_eq!(
            scale.triad_from_degree(1).unwrap().pitched_common_name(),
            "C-major triad"
        );
        assert_eq!(
            scale
                .seventh_chord_from_degree(5)
                .unwrap()
                .pitched_common_name(),
            "G-dominant seventh chord"
        );
    }
}
