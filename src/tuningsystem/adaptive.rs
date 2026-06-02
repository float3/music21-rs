use crate::{
    FloatType, TuningSystem, UnsignedIntegerType,
    tuningsystem::{get_frequency_at, get_ratio_at},
};

/// Adaptive tuning systems whose note frequencies depend on harmonic context.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum AdaptiveTuningSystem {
    /// A recursive tuning system:
    ///
    /// ```text
    /// frequency = base * root_tuning[context] * local_tuning[index]
    /// ```
    ///
    /// `context` is the absolute root index, such as E above C.
    /// `index` is the local interval above that root, such as a major third.
    Recursive {
        /// Tuning system used to place the chord root.
        root_tuning_system: TuningSystem,

        /// Tuning system used inside the chord root.
        local_tuning_system: TuningSystem,
    },
}

/// Recursive just intonation:
///
/// frequency = C_base * JustIntonation[root] * JustIntonation[local_degree]
pub const RECURSIVE_JI: AdaptiveTuningSystem = AdaptiveTuningSystem::Recursive {
    root_tuning_system: TuningSystem::JustIntonation,
    local_tuning_system: TuningSystem::JustIntonation,
};

impl AdaptiveTuningSystem {
    /// Returns the frequency in hertz for a local degree inside a harmonic context.
    ///
    /// For example, in recursive JI:
    ///
    /// ```text
    /// context = 4  // E above C
    /// index   = 4  // major third above E
    ///
    /// frequency = C * 5/4 * 5/4
    ///           = C * 25/16
    /// ```
    pub fn frequency_at(
        self,
        context: FloatType,
        index: FloatType,
        size: Option<UnsignedIntegerType>,
    ) -> FloatType {
        match self {
            Self::Recursive {
                root_tuning_system,
                local_tuning_system,
            } => {
                let root_ratio = get_ratio_at(root_tuning_system, context, size);
                let local_frequency = get_frequency_at(local_tuning_system, index, size);

                root_ratio * local_frequency
            }
        }
    }

    /// Returns cents offset against equal temperament for the resulting absolute pitch.
    ///
    /// This assumes `index` is a local interval above `context`, so the equal-tempered
    /// comparison pitch is `context + index`.
    pub fn cents_at(
        self,
        context: FloatType,
        index: FloatType,
        size: Option<UnsignedIntegerType>,
    ) -> FloatType {
        let octave_size = size.unwrap_or_else(|| match self {
            Self::Recursive {
                root_tuning_system, ..
            } => root_tuning_system.octave_size(),
        });

        let reference_frequency = get_frequency_at(
            TuningSystem::EqualTemperament { octave_size },
            context + index,
            Some(octave_size),
        );

        let comparison_frequency = self.frequency_at(context, index, size);

        1200.0 * (comparison_frequency / reference_frequency).log2()
    }

    /// Returns cents offset against a fixed tuning table for the same absolute pitch.
    ///
    /// Useful for comparing recursive JI against fixed-C JI.
    pub fn cents_vs_fixed_at(
        self,
        fixed_tuning_system: TuningSystem,
        context: FloatType,
        index: FloatType,
        size: Option<UnsignedIntegerType>,
    ) -> FloatType {
        let fixed_frequency = get_frequency_at(fixed_tuning_system, context + index, size);
        let comparison_frequency = self.frequency_at(context, index, size);

        1200.0 * (comparison_frequency / fixed_frequency).log2()
    }
}
