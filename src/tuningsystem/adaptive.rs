use crate::TuningSystem;

pub enum AdaptiveTuningSystems {
    Recursive12Tone {
        tuningsystem: TuningSystem,
        tuningsystem2: TuningSystem,
    },
}
