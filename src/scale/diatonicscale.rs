use crate::{chord::Chord, defaults::IntegerType, error::Result, pitch::Pitch};

use super::concretescale::ConcreteScale;

#[derive(Clone, Debug)]
/// A diatonic scale realized from a tonic, key signature, and mode.
pub struct DiatonicScale {
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

    /// Returns the scale mode.
    pub fn mode(&self) -> &str {
        &self.mode
    }

    /// Returns the tonic pitch.
    pub fn tonic(&self) -> &Pitch {
        self.concrete.tonic()
    }

    /// Returns the pitch at a one-based scale degree.
    pub fn pitch_from_degree(&self, degree: usize) -> Result<Pitch> {
        self.concrete.pitch_from_degree(degree)
    }

    /// Returns pitches from degree 1 through the octave.
    pub fn pitches(&self) -> Result<Vec<Pitch>> {
        self.concrete.pitches()
    }

    /// Builds a diatonic triad from a one-based degree.
    pub fn triad_from_degree(&self, degree: usize) -> Result<Chord> {
        let notes = vec![
            self.pitch_from_degree(degree)?,
            self.pitch_from_degree(degree + 2)?,
            self.pitch_from_degree(degree + 4)?,
        ];
        Chord::new(notes.as_slice())
    }

    /// Builds a diatonic seventh chord from a one-based degree.
    pub fn seventh_chord_from_degree(&self, degree: usize) -> Result<Chord> {
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
