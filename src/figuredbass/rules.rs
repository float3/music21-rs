//! What a figured-bass realization is held to: music21's
//! `figuredBass.rules`.

use crate::defaults::IntegerType;
use crate::pitch::Pitch;

/// The rules a realization keeps, each on or off: which voicings of one
/// chord are acceptable, which moves from one chord to the next are, and
/// how the chords that must resolve a particular way are resolved.
///
/// Every rule starts out as music21's does.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Rules {
    /// Refuse a voicing that leaves out a note of its chord.
    pub forbid_incomplete_possibilities: bool,
    /// How far apart, in semitones, the upper parts may lie; `None` lets
    /// them lie anywhere.
    pub upper_parts_max_semitone_separation: Option<IntegerType>,
    /// Refuse a voicing where a part sings above a part over it.
    pub forbid_voice_crossing: bool,
    /// Refuse parallel fifths.
    pub forbid_parallel_fifths: bool,
    /// Refuse parallel octaves.
    pub forbid_parallel_octaves: bool,
    /// Refuse hidden fifths between the outer parts.
    pub forbid_hidden_fifths: bool,
    /// Refuse hidden octaves between the outer parts.
    pub forbid_hidden_octaves: bool,
    /// Refuse a part moving past where a neighbouring part was.
    pub forbid_voice_overlap: bool,
    /// How far each part named may move, in semitones: `(part, semitones)`,
    /// counting the highest part as one.
    pub part_movement_limits: Vec<(usize, IntegerType)>,
    /// Resolve a dominant seventh as a dominant seventh resolves.
    pub resolve_dominant_seventh_properly: bool,
    /// Resolve a diminished seventh as a diminished seventh resolves.
    pub resolve_diminished_seventh_properly: bool,
    /// Resolve an augmented sixth as an augmented sixth resolves.
    pub resolve_augmented_sixth_properly: bool,
    /// Resolve a diminished seventh to the tonic with its root doubled,
    /// rather than its third, where the inversions do not decide it.
    pub doubled_root_in_dim7: bool,
    /// Hold a special resolution to the rules for single voicings too.
    pub apply_single_possib_rules_to_resolution: bool,
    /// Hold a special resolution to the rules for moving between voicings
    /// too.
    pub apply_consecutive_possib_rules_to_resolution: bool,
    /// In resolving an Italian sixth, let the root resolve only once and
    /// the third not be doubled.
    pub restrict_doublings_in_italian_a6_resolution: bool,
    /// Keep every part but the bass where it was: music21's
    /// `_upperPartsRemainSame`, which a realizer sets for a bass note that
    /// only moves.
    pub upper_parts_remain_same: bool,
    /// Hold each part named to a pitch: `(part, pitch)`, counting the
    /// highest part as one. music21's `_partPitchLimits`.
    pub part_pitch_limits: Vec<(usize, Pitch)>,
    /// The parts that must keep their pitch from one voicing to the next:
    /// music21's `_partsToCheck`.
    pub parts_to_check: Vec<usize>,
}

impl Default for Rules {
    fn default() -> Self {
        Self {
            forbid_incomplete_possibilities: true,
            upper_parts_max_semitone_separation: Some(12),
            forbid_voice_crossing: true,
            forbid_parallel_fifths: true,
            forbid_parallel_octaves: true,
            forbid_hidden_fifths: true,
            forbid_hidden_octaves: true,
            forbid_voice_overlap: true,
            part_movement_limits: Vec::new(),
            resolve_dominant_seventh_properly: true,
            resolve_diminished_seventh_properly: true,
            resolve_augmented_sixth_properly: true,
            doubled_root_in_dim7: false,
            apply_single_possib_rules_to_resolution: false,
            apply_consecutive_possib_rules_to_resolution: false,
            restrict_doublings_in_italian_a6_resolution: true,
            upper_parts_remain_same: false,
            part_pitch_limits: Vec::new(),
            parts_to_check: Vec::new(),
        }
    }
}
