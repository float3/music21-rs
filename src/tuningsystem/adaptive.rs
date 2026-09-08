use crate::{
    FloatType, TuningSystem, UnsignedIntegerType,
    tuningsystem::{get_frequency_at, get_ratio_at},
};

/// Adaptive tuning systems whose note frequencies depend on harmonic context.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[must_use]
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
/// `frequency = C_base * CarlosHarmonic[root] * CarlosHarmonic[local_degree]`
pub const RECURSIVE_JI: AdaptiveTuningSystem = AdaptiveTuningSystem::Recursive {
    root_tuning_system: TuningSystem::CarlosHarmonic,
    local_tuning_system: TuningSystem::CarlosHarmonic,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tuningsystem::{AnyTuningSystem, CN1};

    /// How near two frequencies must be to count as the same pitch.
    const CLOSE: FloatType = 1e-9;

    /// The whole point of an adaptive system, and the example its own
    /// documentation gives: a third above a third is two *just* thirds, not
    /// whatever the fixed table happens to hold eight steps up.
    #[test]
    fn a_third_above_a_third_is_two_just_thirds() {
        // Context 4 is E above C, index 4 a major third above that E.
        let stacked = RECURSIVE_JI.frequency_at(4.0, 4.0, None);
        assert!(
            (stacked - CN1 * 25.0 / 16.0).abs() < CLOSE,
            "5/4 of 5/4 is 25/16, got {stacked}"
        );

        // The fixed table's own eighth degree is 13/8 — a different pitch
        // entirely. That difference is what makes the system adaptive.
        let fixed = RECURSIVE_JI.frequency_at(0.0, 8.0, None);
        assert!((fixed - CN1 * 13.0 / 8.0).abs() < CLOSE, "got {fixed}");
        assert!(
            (stacked - fixed).abs() > 0.4,
            "the recursive and fixed readings of the same step should differ"
        );
    }

    /// Nothing recurses out of the tonic, so the system answers the fixed
    /// table there. Anything else would make the tonic special.
    #[test]
    fn a_context_of_nothing_is_the_fixed_table() {
        for index in [0.0, 4.0, 7.0, 11.0] {
            let adaptive = RECURSIVE_JI.frequency_at(0.0, index, None);
            let fixed = TuningSystem::CarlosHarmonic.frequency_at(index);
            assert!(
                (adaptive - fixed).abs() < CLOSE,
                "at index {index}: {adaptive} against {fixed}"
            );
            // And measured against the very table it came from, it is nought
            // cents away — exactly, not nearly.
            assert_eq!(
                RECURSIVE_JI.cents_vs_fixed_at(TuningSystem::CarlosHarmonic, 0.0, index, None),
                0.0
            );
        }
    }

    /// Against equal temperament, which is what `cents_at` measures.
    #[test]
    fn cents_at_says_how_far_the_stack_lands_from_the_piano() {
        // 25/16 is 772.63 cents; the piano's minor sixth is 800.
        let stacked = RECURSIVE_JI.cents_at(4.0, 4.0, None);
        assert!((stacked + 27.373).abs() < 1e-3, "{stacked}");

        // A just fifth from the tonic is the familiar two cents sharp.
        let fifth = RECURSIVE_JI.cents_at(0.0, 7.0, None);
        assert!((fifth - 1.955).abs() < 1e-3, "{fifth}");

        // The tonic is the tonic in any tuning.
        assert!(RECURSIVE_JI.cents_at(0.0, 0.0, None).abs() < CLOSE);
    }

    /// Against a fixed table for the same written note, which is the
    /// comparison the module exists to make.
    #[test]
    fn cents_vs_fixed_shows_what_recursing_costs() {
        // Two just thirds land 41 cents under five-limit's own minor sixth,
        // which is 8/5. That gap is why a fixed table cannot spell this.
        let against_five_limit =
            RECURSIVE_JI.cents_vs_fixed_at(TuningSystem::FiveLimit, 4.0, 4.0, None);
        assert!(
            (against_five_limit + 41.059).abs() < 1e-3,
            "{against_five_limit}"
        );

        // Measured against equal temperament, `cents_vs_fixed_at` agrees with
        // `cents_at`, since that is the table `cents_at` compares against.
        let twelve = TuningSystem::EqualTemperament { octave_size: 12 };
        for (context, index) in [(4.0, 4.0), (0.0, 7.0), (7.0, 3.0)] {
            assert!(
                (RECURSIVE_JI.cents_vs_fixed_at(twelve, context, index, None)
                    - RECURSIVE_JI.cents_at(context, index, None))
                .abs()
                    < 1e-9,
                "at {context}, {index}"
            );
        }
    }

    /// The octave size may be given or left to the root system to supply.
    #[test]
    fn an_octave_size_given_matches_the_one_it_would_have_chosen() {
        for (context, index) in [(4.0, 4.0), (0.0, 7.0), (2.0, 9.0)] {
            assert!(
                (RECURSIVE_JI.cents_at(context, index, Some(12))
                    - RECURSIVE_JI.cents_at(context, index, None))
                .abs()
                    < 1e-9
            );
            assert!(
                (RECURSIVE_JI.frequency_at(context, index, Some(12))
                    - RECURSIVE_JI.frequency_at(context, index, None))
                .abs()
                    < CLOSE
            );
        }
    }

    /// A system need not recurse on the same table both ways.
    #[test]
    fn the_root_and_the_local_table_are_separate() {
        let mixed = AdaptiveTuningSystem::Recursive {
            root_tuning_system: TuningSystem::PythagoreanTuning,
            local_tuning_system: TuningSystem::CarlosHarmonic,
        };
        // A Pythagorean fifth to place the root, a just third above it.
        let sounded = mixed.frequency_at(7.0, 4.0, None);
        assert!(
            (sounded - CN1 * (3.0 / 2.0) * (5.0 / 4.0)).abs() < CLOSE,
            "{sounded}"
        );
        assert_ne!(mixed, RECURSIVE_JI);
    }

    /// The wrapper that lets a caller hold either kind of system.
    #[test]
    fn an_adaptive_system_answers_through_the_wrapper_too() {
        let any: AnyTuningSystem = RECURSIVE_JI.into();
        assert!(any.is_adaptive());
        assert!(
            (any.frequency_at(4.0, 4.0, None) - RECURSIVE_JI.frequency_at(4.0, 4.0, None)).abs()
                < CLOSE
        );
        assert!(
            (any.cents_at(4.0, 4.0, None) - RECURSIVE_JI.cents_at(4.0, 4.0, None)).abs() < 1e-9
        );

        // A fixed system ignores the context it is handed; an adaptive one
        // does not, which is the whole distinction the wrapper draws.
        let fixed: AnyTuningSystem = TuningSystem::CarlosHarmonic.into();
        assert!(!fixed.is_adaptive());
        assert!(
            (fixed.frequency_at(4.0, 4.0, None) - fixed.frequency_at(0.0, 4.0, None)).abs() < CLOSE
        );
        assert!((any.frequency_at(4.0, 4.0, None) - any.frequency_at(0.0, 4.0, None)).abs() > 0.4);
    }
}
