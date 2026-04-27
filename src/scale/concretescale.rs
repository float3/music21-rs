use std::collections::HashMap;

use crate::{
    defaults::IntegerType,
    exception::{Exception, ExceptionResult},
    pitch::Pitch,
    stepname::StepName,
};

use super::{accidental_modifier_from_alter, altered_steps_from_sharps};

#[derive(Clone, Debug)]
pub(crate) struct ConcreteScale {
    tonic: Pitch,
    altered_steps: HashMap<StepName, IntegerType>,
}

impl ConcreteScale {
    pub(crate) fn new(tonic: Pitch, sharps: IntegerType) -> Self {
        Self {
            tonic,
            altered_steps: altered_steps_from_sharps(sharps),
        }
    }

    pub(crate) fn tonic(&self) -> &Pitch {
        &self.tonic
    }

    pub(crate) fn pitch_from_degree(&self, degree: usize) -> ExceptionResult<Pitch> {
        if degree == 0 {
            return Err(Exception::Ordinal("Scale degree must be >= 1".to_string()));
        }

        let tonic_step_idx = self.tonic.step().step_to_dnn_offset() - 1;
        let tonic_octave = self.tonic.octave().unwrap_or(4);
        let tonic_dnn = tonic_step_idx + 1 + (7 * tonic_octave);
        let target_dnn = tonic_dnn + (degree as IntegerType - 1);

        let (step, octave) = diatonic_number_to_step_and_octave(target_dnn)?;
        let alter = *self.altered_steps.get(&step).unwrap_or(&0);
        let modifier = accidental_modifier_from_alter(alter);
        let name = format!("{step:?}{modifier}{octave}");

        Pitch::new(
            Some(name),
            None,
            None,
            Option::<i8>::None,
            Option::<IntegerType>::None,
            None,
            None,
            None,
            None,
        )
    }

    pub(crate) fn pitches(&self) -> ExceptionResult<Vec<Pitch>> {
        (1..=8)
            .map(|degree| self.pitch_from_degree(degree))
            .collect()
    }
}

fn diatonic_number_to_step_and_octave(dn: IntegerType) -> ExceptionResult<(StepName, IntegerType)> {
    if dn == 0 {
        return Ok((StepName::B, -1));
    }
    if dn > 0 {
        let octave = (dn - 1) / 7;
        let step_number = (dn - 1) - (octave * 7);
        return Ok((StepName::try_from((step_number + 1) as u8)?, octave));
    }

    let octave = (dn as f64 / 7.0).trunc() as IntegerType;
    let step_number = (dn - 1) - (octave * 7);
    Ok((StepName::try_from((step_number + 1) as u8)?, octave - 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pitch(name: &str) -> Pitch {
        Pitch::new(
            Some(name.to_string()),
            None,
            None,
            Option::<i8>::None,
            Option::<IntegerType>::None,
            None,
            None,
            None,
            None,
        )
        .expect("valid pitch")
    }

    #[test]
    fn concrete_scale_c_major() {
        let scale = ConcreteScale::new(pitch("C4"), 0);
        let pitches = scale.pitches().unwrap();
        let names = pitches
            .iter()
            .map(|p| p.name_with_octave())
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["C4", "D4", "E4", "F4", "G4", "A4", "B4", "C5"]);
    }

    #[test]
    fn concrete_scale_d_major_has_f_sharp_and_c_sharp() {
        let scale = ConcreteScale::new(pitch("D4"), 2);
        let names = scale
            .pitches()
            .unwrap()
            .iter()
            .map(|p| p.name_with_octave())
            .collect::<Vec<_>>();
        assert_eq!(
            names,
            vec!["D4", "E4", "F#4", "G4", "A4", "B4", "C#5", "D5"]
        );
    }
}
