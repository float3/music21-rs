//! The features music21 ports from Cory McKay's jSymbolic: music21's
//! `features.jSymbolic`. Each is an [`Extractor`] in [`JSYMBOLIC`], found
//! by its id with [`extractor`].
//!
//! The melodic features (`M`) read the intervals between neighbouring notes
//! of each part; the rhythmic features (`R`) read when each note sounds, in
//! seconds at the piece's tempi, and its meters; the texture features (`T`)
//! read how many parts sound in each chord of the piece chordified; the
//! instrument features (`I`) read the piece partitioned by instrument and
//! each part's General MIDI program; the pitch features (`P`) read the
//! piece's pitches. Several
//! of music21's habits are kept, since the values are the comparison: the
//! pitch variety (`P8`) counts the MIDI numbers sounding but MIDI nought, a
//! tie between two pitches equally common goes to the one sounded first, and
//! a melody with no arpeggiation at all (`M8`) is refused as one with no
//! notes.

use super::{DataInstance, Extractor, Value};
use crate::stream::StreamElement;
use crate::{
    defaults::{FloatType, IntegerType},
    error::{Error, Result},
};

/// Every jSymbolic extractor music21 implements, in music21's order.
pub const JSYMBOLIC: &[Extractor] = &[
    Extractor {
        id: "M1",
        name: "Melodic Interval Histogram",
        description: "A features array with bins corresponding to the values of the melodic interval histogram.",
        dimensions: 128,
        discrete: true,
        normalize: true,
        process: melodic_interval_histogram,
    },
    Extractor {
        id: "M2",
        name: "Average Melodic Interval",
        description: "Average melodic interval (in semitones).",
        dimensions: 1,
        discrete: true,
        normalize: false,
        process: average_melodic_interval,
    },
    Extractor {
        id: "M3",
        name: "Most Common Melodic Interval",
        description: "Melodic interval with the highest frequency.",
        dimensions: 1,
        discrete: true,
        normalize: false,
        process: most_common_melodic_interval,
    },
    Extractor {
        id: "M4",
        name: "Distance Between Most Common Melodic Intervals",
        description: "Absolute value of the difference between the most common melodic interval and the second most common melodic interval.",
        dimensions: 1,
        discrete: true,
        normalize: false,
        process: distance_between_most_common_melodic_intervals,
    },
    Extractor {
        id: "M5",
        name: "Most Common Melodic Interval Prevalence",
        description: "Fraction of melodic intervals that belong to the most common interval.",
        dimensions: 1,
        discrete: true,
        normalize: false,
        process: most_common_melodic_interval_prevalence,
    },
    Extractor {
        id: "M6",
        name: "Relative Strength of Most Common Intervals",
        description: "Fraction of melodic intervals that belong to the second most common interval divided by the fraction of melodic intervals belonging to the most common interval.",
        dimensions: 1,
        discrete: true,
        normalize: false,
        process: relative_strength_of_most_common_intervals,
    },
    Extractor {
        id: "M7",
        name: "Number of Common Melodic Intervals",
        description: "Number of melodic intervals that represent at least 9% of all melodic intervals.",
        dimensions: 1,
        discrete: true,
        normalize: false,
        process: number_of_common_melodic_intervals,
    },
    Extractor {
        id: "M8",
        name: "Amount of Arpeggiation",
        description: "Fraction of horizontal intervals that are repeated notes, minor thirds, major thirds, perfect fifths, minor sevenths, major sevenths, octaves, minor tenths or major tenths.",
        dimensions: 1,
        discrete: true,
        normalize: false,
        process: amount_of_arpeggiation,
    },
    Extractor {
        id: "M9",
        name: "Repeated Notes",
        description: "Fraction of notes that are repeated melodically.",
        dimensions: 1,
        discrete: true,
        normalize: false,
        process: repeated_notes,
    },
    Extractor {
        id: "m10",
        name: "Chromatic Motion",
        description: "Fraction of melodic intervals corresponding to a semi-tone.",
        dimensions: 1,
        discrete: true,
        normalize: false,
        process: chromatic_motion,
    },
    Extractor {
        id: "M11",
        name: "Stepwise Motion",
        description: "Fraction of melodic intervals that corresponded to a minor or major second.",
        dimensions: 1,
        discrete: true,
        normalize: false,
        process: stepwise_motion,
    },
    Extractor {
        id: "M12",
        name: "Melodic Thirds",
        description: "Fraction of melodic intervals that are major or minor thirds.",
        dimensions: 1,
        discrete: true,
        normalize: false,
        process: melodic_thirds,
    },
    Extractor {
        id: "M13",
        name: "Melodic Fifths",
        description: "Fraction of melodic intervals that are perfect fifths.",
        dimensions: 1,
        discrete: true,
        normalize: false,
        process: melodic_fifths,
    },
    Extractor {
        id: "M14",
        name: "Melodic Tritones",
        description: "Fraction of melodic intervals that are tritones.",
        dimensions: 1,
        discrete: true,
        normalize: false,
        process: melodic_tritones,
    },
    Extractor {
        id: "M15",
        name: "Melodic Octaves",
        description: "Fraction of melodic intervals that are octaves.",
        dimensions: 1,
        discrete: true,
        normalize: false,
        process: melodic_octaves,
    },
    Extractor {
        id: "m17",
        name: "Direction of Motion",
        description: "Fraction of melodic intervals that are rising rather than falling.",
        dimensions: 1,
        discrete: true,
        normalize: false,
        process: direction_of_motion,
    },
    Extractor {
        id: "M18",
        name: "Duration of Melodic Arcs",
        description: "Average number of notes that separate melodic peaks and troughs in any part. This is calculated as the total number of intervals (not counting unisons) divided by the number of times the melody changes direction.",
        dimensions: 1,
        discrete: true,
        normalize: false,
        process: duration_of_melodic_arcs,
    },
    Extractor {
        id: "M19",
        name: "Size of Melodic Arcs",
        description: "Average span (in semitones) between melodic peaks and troughs in any part. Each time the melody changes direction begins a new arc. The average size ofmelodic arcs is defined as the total size of melodicintervals between changes of directions - or betweenthe start of the melody and the first change ofdirection - divided by the number of direction changes.",
        dimensions: 1,
        discrete: true,
        normalize: false,
        process: size_of_melodic_arcs,
    },
    Extractor {
        id: "I1",
        name: "Pitched Instruments Present",
        description: "Which pitched General MIDI Instruments are present. There is one entry for each instrument, which is set to 1.0 if there is at least one Note On in the recording corresponding to the instrument and to 0.0 if there is not.",
        dimensions: 128,
        discrete: true,
        normalize: false,
        process: pitched_instruments_present,
    },
    Extractor {
        id: "I3",
        name: "Note Prevalence of Pitched Instruments",
        description: "The fraction of (pitched) notes played by each General MIDI Instrument. There is one entry for each instrument, which is set to the number of Note Ons played using the corresponding MIDI patch divided by the total number of Note Ons in the recording.",
        dimensions: 128,
        discrete: true,
        normalize: false,
        process: note_prevalence_of_pitched_instruments,
    },
    Extractor {
        id: "I6",
        name: "Variability of Note Prevalence of Pitched Instruments",
        description: "Standard deviation of the fraction of Note Ons played by each (pitched) General MIDI instrument that is used to play at least one note.",
        dimensions: 1,
        discrete: true,
        normalize: false,
        process: variability_of_note_prevalence_of_pitched_instruments,
    },
    Extractor {
        id: "I8",
        name: "Number of Pitched Instruments",
        description: "Total number of General MIDI patches that are used to play at least one note.",
        dimensions: 1,
        discrete: true,
        normalize: false,
        process: number_of_pitched_instruments,
    },
    Extractor {
        id: "I11",
        name: "String Keyboard Fraction",
        description: "Fraction of all Note Ons belonging to string keyboard patches (General MIDI patches 1 to 8).",
        dimensions: 1,
        discrete: true,
        normalize: false,
        process: string_keyboard_fraction,
    },
    Extractor {
        id: "I12",
        name: "Acoustic Guitar Fraction",
        description: "Fraction of all Note Ons belonging to acoustic guitar patches (General MIDI patches 25 and 26).",
        dimensions: 1,
        discrete: true,
        normalize: false,
        process: acoustic_guitar_fraction,
    },
    Extractor {
        id: "I13",
        name: "Electric Guitar Fraction",
        description: "Fraction of all Note Ons belonging to electric guitar patches (General MIDI patches 27 to 32).",
        dimensions: 1,
        discrete: true,
        normalize: false,
        process: electric_guitar_fraction,
    },
    Extractor {
        id: "I14",
        name: "Violin Fraction",
        description: "Fraction of all Note Ons belonging to violin patches (General MIDI patches 41 or 111).",
        dimensions: 1,
        discrete: true,
        normalize: false,
        process: violin_fraction,
    },
    Extractor {
        id: "I15",
        name: "Saxophone Fraction",
        description: "Fraction of all Note Ons belonging to saxophone patches (General MIDI patches 65 through 68).",
        dimensions: 1,
        discrete: true,
        normalize: false,
        process: saxophone_fraction,
    },
    Extractor {
        id: "I16",
        name: "Brass Fraction",
        description: "Fraction of all Note Ons belonging to brass patches (General MIDI patches 57 through 68).",
        dimensions: 1,
        discrete: true,
        normalize: false,
        process: brass_fraction,
    },
    Extractor {
        id: "I17",
        name: "Woodwinds Fraction",
        description: "Fraction of all Note Ons belonging to woodwind patches (General MIDI patches 69 through 76).",
        dimensions: 1,
        discrete: true,
        normalize: false,
        process: woodwinds_fraction,
    },
    Extractor {
        id: "I18",
        name: "Orchestral Strings Fraction",
        description: "Fraction of all Note Ons belonging to orchestral strings patches (General MIDI patches 41 or 47).",
        dimensions: 1,
        discrete: true,
        normalize: false,
        process: orchestral_strings_fraction,
    },
    Extractor {
        id: "I19",
        name: "String Ensemble Fraction",
        description: "Fraction of all Note Ons belonging to string ensemble patches (General MIDI patches 49 to 52).",
        dimensions: 1,
        discrete: true,
        normalize: false,
        process: string_ensemble_fraction,
    },
    Extractor {
        id: "I20",
        name: "Electric Instrument Fraction",
        description: "Fraction of all Note Ons belonging to electric instrument patches (General MIDI patches 5, 6, 17, 19, 27 to 32 or 34 to 40).",
        dimensions: 1,
        discrete: true,
        normalize: false,
        process: electric_instrument_fraction,
    },
    Extractor {
        id: "R15",
        name: "Note Density",
        description: "Average number of notes per second.",
        dimensions: 1,
        discrete: true,
        normalize: false,
        process: note_density,
    },
    Extractor {
        id: "R17",
        name: "Average Note Duration",
        description: "Average duration of notes in seconds.",
        dimensions: 1,
        discrete: true,
        normalize: false,
        process: average_note_duration,
    },
    Extractor {
        id: "R18",
        name: "Variability of Note Duration",
        description: "Standard deviation of note durations in seconds.",
        dimensions: 1,
        discrete: true,
        normalize: false,
        process: variability_of_note_duration,
    },
    Extractor {
        id: "R19",
        name: "Maximum Note Duration",
        description: "Duration of the longest note (in seconds).",
        dimensions: 1,
        discrete: true,
        normalize: false,
        process: maximum_note_duration,
    },
    Extractor {
        id: "R20",
        name: "Minimum Note Duration",
        description: "Duration of the shortest note (in seconds).",
        dimensions: 1,
        discrete: true,
        normalize: false,
        process: minimum_note_duration,
    },
    Extractor {
        id: "R21",
        name: "Staccato Incidence",
        description: "Number of notes with durations of less than a 10th of a second divided by the total number of notes in the recording.",
        dimensions: 1,
        discrete: true,
        normalize: false,
        process: staccato_incidence,
    },
    Extractor {
        id: "R22",
        name: "Average Time Between Attacks",
        description: "Average time in seconds between Note On events (regardless of channel).",
        dimensions: 1,
        discrete: true,
        normalize: false,
        process: average_time_between_attacks,
    },
    Extractor {
        id: "R23",
        name: "Variability of Time Between Attacks",
        description: "Standard deviation of the times, in seconds, between Note On events (regardless of channel).",
        dimensions: 1,
        discrete: true,
        normalize: false,
        process: variability_of_time_between_attacks,
    },
    Extractor {
        id: "R24",
        name: "Average Time Between Attacks For Each Voice",
        description: "Average of average times in seconds between Note On events on individual channels that contain at least one note.",
        dimensions: 1,
        discrete: true,
        normalize: false,
        process: average_time_between_attacks_for_each_voice,
    },
    Extractor {
        id: "R25",
        name: "Average Variability of Time Between Attacks For Each Voice",
        description: "Average standard deviation, in seconds, of time between Note On events on individual channels that contain at least one note.",
        dimensions: 1,
        discrete: true,
        normalize: false,
        process: average_variability_of_time_between_attacks_for_each_voice,
    },
    Extractor {
        id: "R30",
        name: "Initial Tempo",
        description: "Tempo in beats per minute at the start of the recording.",
        dimensions: 1,
        discrete: true,
        normalize: false,
        process: initial_tempo,
    },
    Extractor {
        id: "R31",
        name: "Initial Time Signature",
        description: "A feature array with two elements. The first is the numerator of the first occurring time signature and the second is the denominator of the first occurring time signature. Both are set to 0 if no time signature is present.",
        dimensions: 2,
        discrete: true,
        normalize: false,
        process: initial_time_signature,
    },
    Extractor {
        id: "R32",
        name: "Compound Or Simple Meter",
        description: "Set to 1 if the initial meter is compound (numerator of time signature is greater than or equal to 6 and is evenly divisible by 3) and to 0 if it is simple (if the above condition is not fulfilled).",
        dimensions: 1,
        discrete: true,
        normalize: false,
        process: compound_or_simple_meter,
    },
    Extractor {
        id: "R33",
        name: "Triple Meter",
        description: "Set to 1 if numerator of initial time signature is 3, set to 0 otherwise.",
        dimensions: 1,
        discrete: true,
        normalize: false,
        process: triple_meter,
    },
    Extractor {
        id: "R34",
        name: "Quintuple Meter",
        description: "Set to 1 if numerator of initial time signature is 5, set to 0 otherwise.",
        dimensions: 1,
        discrete: true,
        normalize: false,
        process: quintuple_meter,
    },
    Extractor {
        id: "R35",
        name: "Changes of Meter",
        description: "Set to 1 if the time signature is changed one or more times during the recording.",
        dimensions: 1,
        discrete: true,
        normalize: false,
        process: changes_of_meter,
    },
    Extractor {
        id: "R36",
        name: "Duration",
        description: "The total duration in seconds of the music.",
        dimensions: 1,
        discrete: false,
        normalize: false,
        process: duration,
    },
    Extractor {
        id: "T1",
        name: "Maximum Number of Independent Voices",
        description: "Maximum number of different channels in which notes have sounded simultaneously. Here, Parts are treated as channels.",
        dimensions: 1,
        discrete: true,
        normalize: false,
        process: maximum_number_of_independent_voices,
    },
    Extractor {
        id: "T2",
        name: "Average Number of Independent Voices",
        description: "Average number of different channels in which notes have sounded simultaneously. Rests are not included in this calculation. Here, Parts are treated as voices.",
        dimensions: 1,
        discrete: true,
        normalize: false,
        process: average_number_of_independent_voices,
    },
    Extractor {
        id: "T3",
        name: "Variability of Number of Independent Voices",
        description: "Standard deviation of number of different channels in which notes have sounded simultaneously. Rests are not included in this calculation.",
        dimensions: 1,
        discrete: true,
        normalize: false,
        process: variability_of_number_of_independent_voices,
    },
    Extractor {
        id: "P1",
        name: "Most Common Pitch Prevalence",
        description: "Fraction of Note Ons corresponding to the most common pitch.",
        dimensions: 1,
        discrete: false,
        normalize: false,
        process: most_common_pitch_prevalence,
    },
    Extractor {
        id: "P2",
        name: "Most Common Pitch Class Prevalence",
        description: "Fraction of Note Ons corresponding to the most common pitch class.",
        dimensions: 1,
        discrete: false,
        normalize: false,
        process: most_common_pitch_class_prevalence,
    },
    Extractor {
        id: "P3",
        name: "Relative Strength of Top Pitches",
        description: "The frequency of the 2nd most common pitch divided by the frequency of the most common pitch.",
        dimensions: 1,
        discrete: false,
        normalize: false,
        process: relative_strength_of_top_pitches,
    },
    Extractor {
        id: "P4",
        name: "Relative Strength of Top Pitch Classes",
        description: "The frequency of the 2nd most common pitch class divided by the frequency of the most common pitch class.",
        dimensions: 1,
        discrete: false,
        normalize: false,
        process: relative_strength_of_top_pitch_classes,
    },
    Extractor {
        id: "P5",
        name: "Interval Between Strongest Pitches",
        description: "Absolute value of the difference between the pitches of the two most common MIDI pitches.",
        dimensions: 1,
        discrete: true,
        normalize: false,
        process: interval_between_strongest_pitches,
    },
    Extractor {
        id: "P6",
        name: "Interval Between Strongest Pitch Classes",
        description: "Absolute value of the difference between the pitch classes of the two most common MIDI pitch classes.",
        dimensions: 1,
        discrete: true,
        normalize: false,
        process: interval_between_strongest_pitch_classes,
    },
    Extractor {
        id: "P7",
        name: "Number of Common Pitches",
        description: "Number of pitches that account individually for at least 9% of all notes.",
        dimensions: 1,
        discrete: true,
        normalize: false,
        process: number_of_common_pitches,
    },
    Extractor {
        id: "P8",
        name: "Pitch Variety",
        description: "Number of pitches used at least once.",
        dimensions: 1,
        discrete: true,
        normalize: false,
        process: pitch_variety,
    },
    Extractor {
        id: "P9",
        name: "Pitch Class Variety",
        description: "Number of pitch classes used at least once.",
        dimensions: 1,
        discrete: true,
        normalize: false,
        process: pitch_class_variety,
    },
    Extractor {
        id: "P10",
        name: "Range",
        description: "Difference between highest and lowest pitches.",
        dimensions: 1,
        discrete: true,
        normalize: false,
        process: range,
    },
    Extractor {
        id: "P11",
        name: "Most Common Pitch",
        description: "Bin label of the most common pitch.",
        dimensions: 1,
        discrete: false,
        normalize: false,
        process: most_common_pitch,
    },
    Extractor {
        id: "P12",
        name: "Primary Register",
        description: "Average MIDI pitch.",
        dimensions: 1,
        discrete: false,
        normalize: false,
        process: primary_register,
    },
    Extractor {
        id: "P13",
        name: "Importance of Bass Register",
        description: "Fraction of Note Ons between MIDI pitches 0 and 54.",
        dimensions: 1,
        discrete: false,
        normalize: false,
        process: importance_of_bass_register,
    },
    Extractor {
        id: "P14",
        name: "Importance of Middle Register",
        description: "Fraction of Note Ons between MIDI pitches 55 and 72.",
        dimensions: 1,
        discrete: false,
        normalize: false,
        process: importance_of_middle_register,
    },
    Extractor {
        id: "P15",
        name: "Importance of High Register",
        description: "Fraction of Note Ons between MIDI pitches 73 and 127.",
        dimensions: 1,
        discrete: false,
        normalize: false,
        process: importance_of_high_register,
    },
    Extractor {
        id: "P16",
        name: "Most Common Pitch Class",
        description: "Bin label of the most common pitch class.",
        dimensions: 1,
        discrete: true,
        normalize: false,
        process: most_common_pitch_class,
    },
    Extractor {
        id: "P19",
        name: "Basic Pitch Histogram",
        description: "A features array with bins corresponding to the values of the basic pitch histogram.",
        dimensions: 128,
        discrete: true,
        normalize: true,
        process: basic_pitch_histogram,
    },
    Extractor {
        id: "P20",
        name: "Pitch Class Distribution",
        description: "A feature array with 12 entries where the first holds the frequency of the bin of the pitch class histogram with the highest frequency, and the following entries holding the successive bins of the histogram, wrapping around if necessary.",
        dimensions: 12,
        discrete: false,
        normalize: true,
        process: pitch_class_distribution,
    },
    Extractor {
        id: "P21",
        name: "Fifths Pitch Histogram",
        description: "A feature array with bins corresponding to the values of the 5ths pitch class histogram.",
        dimensions: 12,
        discrete: true,
        normalize: true,
        process: fifths_pitch_histogram,
    },
    Extractor {
        id: "P22",
        name: "Quality",
        description: "Set to 0 if the key signature indicates that a recording is major, set to 1 if it indicates that it is minor and set to 0 if key signature is unknown.",
        dimensions: 1,
        discrete: true,
        normalize: false,
        process: quality,
    },
];

/// The jSymbolic extractor with this id, such as `P10` or music21's `m10`.
pub fn extractor(id: &str) -> Option<&'static Extractor> {
    JSYMBOLIC.iter().find(|extractor| extractor.id == id)
}

fn lacks_notes() -> Error {
    Error::Feature("input lacks notes".to_string())
}

/// The index of the first of the greatest values: Python's
/// `list.index(max(list))`.
fn first_max(values: &[usize]) -> usize {
    let most = values.iter().copied().max().unwrap_or(0);
    values.iter().position(|value| *value == most).unwrap_or(0)
}

/// A counter's entries most common first, as `Counter.most_common` gives
/// them: a tie goes to the one counted first.
fn most_common(counts: &[(IntegerType, usize)]) -> Vec<(IntegerType, usize)> {
    let mut sorted = counts.to_vec();
    sorted.sort_by_key(|entry| std::cmp::Reverse(entry.1));
    sorted
}

/// The share of the intervals counted that are of the sizes given.
fn share_of(histogram: &[usize], targets: &[usize]) -> Result<FloatType> {
    let total: usize = histogram.iter().sum();
    if total == 0 {
        return Err(lacks_notes());
    }
    let count: usize = targets.iter().map(|target| histogram[*target]).sum();
    Ok(count as FloatType / total as FloatType)
}

fn melodic_interval_histogram(data: &DataInstance, vector: &mut [Value]) -> Result<()> {
    for (value, count) in vector.iter_mut().zip(data.midi_interval_histogram()) {
        *value = (*count).into();
    }
    Ok(())
}

fn average_melodic_interval(data: &DataInstance, vector: &mut [Value]) -> Result<()> {
    let histogram = data.midi_interval_histogram();
    let count: usize = histogram.iter().sum();
    if count == 0 {
        return Err(lacks_notes());
    }
    let total: usize = histogram
        .iter()
        .enumerate()
        .map(|(size, count)| size * count)
        .sum();
    vector[0] = (total as FloatType / count as FloatType).into();
    Ok(())
}

fn most_common_melodic_interval(data: &DataInstance, vector: &mut [Value]) -> Result<()> {
    vector[0] = first_max(data.midi_interval_histogram()).into();
    Ok(())
}

fn distance_between_most_common_melodic_intervals(
    data: &DataInstance,
    vector: &mut [Value],
) -> Result<()> {
    let mut histogram = data.midi_interval_histogram().to_vec();
    let first = first_max(&histogram);
    histogram[first] = 0;
    let second = first_max(&histogram);
    vector[0] = first.abs_diff(second).into();
    Ok(())
}

fn most_common_melodic_interval_prevalence(
    data: &DataInstance,
    vector: &mut [Value],
) -> Result<()> {
    let histogram = data.midi_interval_histogram();
    let most = histogram.iter().copied().max().unwrap_or(0);
    let count: usize = histogram.iter().sum();
    if count == 0 {
        return Err(lacks_notes());
    }
    vector[0] = (most as FloatType / count as FloatType).into();
    Ok(())
}

fn relative_strength_of_most_common_intervals(
    data: &DataInstance,
    vector: &mut [Value],
) -> Result<()> {
    let mut histogram = data.midi_interval_histogram().to_vec();
    let count: usize = histogram.iter().sum();
    if count == 0 {
        return Err(lacks_notes());
    }
    let first = first_max(&histogram);
    let most = histogram[first];
    histogram[first] = 0;
    let second = histogram.iter().copied().max().unwrap_or(0);
    let count = count as FloatType;
    vector[0] = ((second as FloatType / count) / (most as FloatType / count)).into();
    Ok(())
}

fn number_of_common_melodic_intervals(data: &DataInstance, vector: &mut [Value]) -> Result<()> {
    let histogram = data.midi_interval_histogram();
    let total: usize = histogram.iter().sum();
    if total == 0 {
        return Err(lacks_notes());
    }
    vector[0] = histogram
        .iter()
        .filter(|count| **count as FloatType / total as FloatType >= 0.09)
        .count()
        .into();
    Ok(())
}

fn amount_of_arpeggiation(data: &DataInstance, vector: &mut [Value]) -> Result<()> {
    let histogram = data.midi_interval_histogram();
    let total: usize = histogram.iter().sum();
    if total == 0 {
        return Ok(());
    }
    let count: usize = [0, 3, 4, 7, 10, 11, 12, 15, 16]
        .iter()
        .map(|target| histogram[*target])
        .sum();
    // music21 takes a melody with no arpeggiation at all for one with no
    // notes.
    if count == 0 {
        return Err(lacks_notes());
    }
    vector[0] = (count as FloatType / total as FloatType).into();
    Ok(())
}

fn repeated_notes(data: &DataInstance, vector: &mut [Value]) -> Result<()> {
    let histogram = data.midi_interval_histogram();
    let total: usize = histogram.iter().sum();
    if total == 0 {
        return Ok(());
    }
    vector[0] = (histogram[0] as FloatType / total as FloatType).into();
    Ok(())
}

fn chromatic_motion(data: &DataInstance, vector: &mut [Value]) -> Result<()> {
    vector[0] = share_of(data.midi_interval_histogram(), &[1])?.into();
    Ok(())
}

fn stepwise_motion(data: &DataInstance, vector: &mut [Value]) -> Result<()> {
    vector[0] = share_of(data.midi_interval_histogram(), &[1, 2])?.into();
    Ok(())
}

fn melodic_thirds(data: &DataInstance, vector: &mut [Value]) -> Result<()> {
    vector[0] = share_of(data.midi_interval_histogram(), &[3, 4])?.into();
    Ok(())
}

fn melodic_fifths(data: &DataInstance, vector: &mut [Value]) -> Result<()> {
    vector[0] = share_of(data.midi_interval_histogram(), &[7])?.into();
    Ok(())
}

fn melodic_tritones(data: &DataInstance, vector: &mut [Value]) -> Result<()> {
    vector[0] = share_of(data.midi_interval_histogram(), &[6])?.into();
    Ok(())
}

fn melodic_octaves(data: &DataInstance, vector: &mut [Value]) -> Result<()> {
    vector[0] = share_of(
        data.midi_interval_histogram(),
        &[12, 24, 48, 60, 72, 84, 96, 108, 120],
    )?
    .into();
    Ok(())
}

/// The contour of each part of a score, or of the piece where it is not
/// one.
fn contours(data: &DataInstance) -> Vec<&[IntegerType]> {
    if data.parts_count() > 0 {
        data.parts()[..data.parts_count()]
            .iter()
            .map(DataInstance::contour_list)
            .collect()
    } else {
        vec![data.contour_list()]
    }
}

fn direction_of_motion(data: &DataInstance, vector: &mut [Value]) -> Result<()> {
    let (mut rising, mut falling) = (0usize, 0usize);
    for contour in contours(data) {
        for step in contour {
            if *step > 0 {
                rising += 1;
            } else if *step < 0 {
                falling += 1;
            }
        }
    }
    if rising + falling == 0 {
        return Err(lacks_notes());
    }
    vector[0] = (rising as FloatType / (falling + rising) as FloatType).into();
    Ok(())
}

fn duration_of_melodic_arcs(data: &DataInstance, vector: &mut [Value]) -> Result<()> {
    let (mut changes, mut moving) = (0usize, 0usize);
    for contour in contours(data) {
        let mut direction = 0;
        for step in contour {
            if *step != 0 {
                moving += 1;
            }
            match direction {
                1 if *step < 0 => {
                    changes += 1;
                    direction = -1;
                }
                -1 if *step > 0 => {
                    changes += 1;
                    direction = 1;
                }
                0 => direction = step.signum(),
                _ => {}
            }
        }
    }
    if changes > 0 {
        vector[0] = (moving as FloatType / changes as FloatType).into();
    }
    Ok(())
}

fn size_of_melodic_arcs(data: &DataInstance, vector: &mut [Value]) -> Result<()> {
    let (mut changes, mut total) = (0usize, 0i64);
    for contour in contours(data) {
        let mut direction = 0;
        let mut arc = 0i64;
        for step in contour {
            let size = i64::from(step.abs());
            match direction {
                1 if *step > 0 => arc += size,
                -1 if *step < 0 => arc += size,
                1 | -1 if *step != 0 => {
                    total += arc;
                    changes += 1;
                    direction = step.signum();
                    arc = size;
                }
                0 if *step != 0 => {
                    direction = step.signum();
                    arc += size;
                }
                _ => {}
            }
        }
    }
    if changes > 0 {
        vector[0] = (total as FloatType / changes as FloatType).into();
    }
    Ok(())
}

fn total_of(counts: &[(IntegerType, usize)]) -> usize {
    counts.iter().map(|(_, count)| count).sum()
}

fn most_common_pitch_prevalence(data: &DataInstance, vector: &mut [Value]) -> Result<()> {
    let histogram = data.midi_pitch_histogram();
    if histogram.is_empty() {
        return Err(lacks_notes());
    }
    let most = histogram.iter().map(|(_, count)| *count).max().unwrap_or(0);
    vector[0] = (most as FloatType / total_of(histogram) as FloatType).into();
    Ok(())
}

fn most_common_pitch_class_prevalence(data: &DataInstance, vector: &mut [Value]) -> Result<()> {
    let histogram = data.pitch_class_histogram();
    let class = first_max(histogram);
    let total: usize = histogram.iter().sum();
    if total == 0 {
        return Err(lacks_notes());
    }
    vector[0] = (histogram[class] as FloatType / total as FloatType).into();
    Ok(())
}

fn relative_strength_of_top_pitches(data: &DataInstance, vector: &mut [Value]) -> Result<()> {
    vector[0] = match most_common(data.midi_pitch_histogram()).as_slice() {
        [first, second, ..] => (second.1 as FloatType / first.1 as FloatType).into(),
        _ => Value::Float(0.0),
    };
    Ok(())
}

fn relative_strength_of_top_pitch_classes(data: &DataInstance, vector: &mut [Value]) -> Result<()> {
    let mut histogram = *data.pitch_class_histogram();
    let first = first_max(&histogram);
    let most = histogram[first];
    if most == 0 {
        return Err(lacks_notes());
    }
    histogram[first] = 0;
    let second = histogram[first_max(&histogram)];
    vector[0] = (second as FloatType / most as FloatType).into();
    Ok(())
}

fn interval_between_strongest_pitches(data: &DataInstance, vector: &mut [Value]) -> Result<()> {
    vector[0] = match most_common(data.midi_pitch_histogram()).as_slice() {
        [first, second, ..] => (second.0 - first.0).abs().into(),
        _ => Value::Float(0.0),
    };
    Ok(())
}

fn interval_between_strongest_pitch_classes(
    data: &DataInstance,
    vector: &mut [Value],
) -> Result<()> {
    let mut histogram = *data.pitch_class_histogram();
    let first = first_max(&histogram);
    histogram[first] = 0;
    let second = first_max(&histogram);
    vector[0] = first.abs_diff(second).into();
    Ok(())
}

fn number_of_common_pitches(data: &DataInstance, vector: &mut [Value]) -> Result<()> {
    let histogram = data.midi_pitch_histogram();
    let total = total_of(histogram) as FloatType;
    vector[0] = histogram
        .iter()
        .filter(|(_, count)| *count as FloatType / total >= 0.09)
        .count()
        .into();
    Ok(())
}

fn pitch_variety(data: &DataInstance, vector: &mut [Value]) -> Result<()> {
    // music21 walks the counter's keys, the MIDI numbers, and counts those
    // at least one, which leaves out MIDI nought.
    vector[0] = data
        .midi_pitch_histogram()
        .iter()
        .filter(|(midi, _)| *midi >= 1)
        .count()
        .into();
    Ok(())
}

fn pitch_class_variety(data: &DataInstance, vector: &mut [Value]) -> Result<()> {
    vector[0] = data
        .pitch_class_histogram()
        .iter()
        .filter(|count| **count >= 1)
        .count()
        .into();
    Ok(())
}

fn range(data: &DataInstance, vector: &mut [Value]) -> Result<()> {
    let histogram = data.midi_pitch_histogram();
    let lowest = histogram.iter().map(|(midi, _)| *midi).min();
    let highest = histogram.iter().map(|(midi, _)| *midi).max();
    let (Some(lowest), Some(highest)) = (lowest, highest) else {
        return Err(lacks_notes());
    };
    vector[0] = (highest - lowest).into();
    Ok(())
}

fn most_common_pitch(data: &DataInstance, vector: &mut [Value]) -> Result<()> {
    vector[0] = most_common(data.midi_pitch_histogram())
        .first()
        .map_or(Value::Float(0.0), |(midi, _)| (*midi).into());
    Ok(())
}

fn primary_register(data: &DataInstance, vector: &mut [Value]) -> Result<()> {
    let pitches = data.pitches();
    if pitches.is_empty() {
        return Err(lacks_notes());
    }
    let heights: Vec<FloatType> = pitches.iter().map(|pitch| pitch.ps()).collect();
    vector[0] = crate::statistics::mean(&heights).into();
    Ok(())
}

/// The share of the notes sounding a MIDI number `within` accepts.
fn register_share(data: &DataInstance, within: impl Fn(IntegerType) -> bool) -> Result<FloatType> {
    let histogram = data.midi_pitch_histogram();
    if histogram.is_empty() {
        return Err(lacks_notes());
    }
    let matched: usize = histogram
        .iter()
        .filter(|(midi, _)| within(*midi))
        .map(|(_, count)| count)
        .sum();
    Ok(matched as FloatType / total_of(histogram) as FloatType)
}

fn importance_of_bass_register(data: &DataInstance, vector: &mut [Value]) -> Result<()> {
    vector[0] = register_share(data, |midi| midi <= 54)?.into();
    Ok(())
}

fn importance_of_middle_register(data: &DataInstance, vector: &mut [Value]) -> Result<()> {
    vector[0] = register_share(data, |midi| (55..=72).contains(&midi))?.into();
    Ok(())
}

fn importance_of_high_register(data: &DataInstance, vector: &mut [Value]) -> Result<()> {
    vector[0] = register_share(data, |midi| midi >= 73)?.into();
    Ok(())
}

fn most_common_pitch_class(data: &DataInstance, vector: &mut [Value]) -> Result<()> {
    vector[0] = first_max(data.pitch_class_histogram()).into();
    Ok(())
}

fn basic_pitch_histogram(data: &DataInstance, vector: &mut [Value]) -> Result<()> {
    for (midi, count) in data.midi_pitch_histogram() {
        if let Some(value) = usize::try_from(*midi)
            .ok()
            .and_then(|midi| vector.get_mut(midi))
        {
            *value = (*count).into();
        }
    }
    Ok(())
}

fn pitch_class_distribution(data: &DataInstance, vector: &mut [Value]) -> Result<()> {
    let histogram = data.pitch_class_histogram();
    let most = first_max(histogram);
    for (class, count) in histogram.iter().enumerate() {
        vector[(class + 12 - most) % 12] = (*count).into();
    }
    Ok(())
}

/// Where each pitch class goes in a histogram ordered by fifths.
const FIFTHS: [usize; 12] = [0, 7, 2, 9, 4, 11, 6, 1, 8, 3, 10, 5];

fn fifths_pitch_histogram(data: &DataInstance, vector: &mut [Value]) -> Result<()> {
    for (class, count) in data.pitch_class_histogram().iter().enumerate() {
        vector[FIFTHS[class]] = (*count).into();
    }
    Ok(())
}

fn quality(data: &DataInstance, vector: &mut [Value]) -> Result<()> {
    vector[0] = data
        .key_modes()
        .iter()
        .find_map(|mode| match mode.as_str() {
            "major" => Some(Value::Integer(0)),
            "minor" => Some(Value::Integer(1)),
            _ => None,
        })
        .unwrap_or(Value::Integer(0));
    Ok(())
}

/// When each note of a piece starts, in seconds, earliest first.
fn onsets(data: &DataInstance) -> Result<Vec<FloatType>> {
    let mut onsets: Vec<FloatType> = data.seconds_map()?.iter().map(|note| note.offset).collect();
    onsets.sort_by(FloatType::total_cmp);
    Ok(onsets)
}

/// The time between each onset and the next, leaving out notes struck
/// together: music21's `isclose(dif, 0.0, abs_tol=1e-7)`.
fn time_between(onsets: &[FloatType]) -> Vec<FloatType> {
    onsets
        .windows(2)
        .map(|pair| pair[1] - pair[0])
        .filter(|gap| gap.abs() > 1e-7)
        .collect()
}

/// The onsets of each part of a score, or of the piece where it is not one.
fn onsets_by_part(data: &DataInstance) -> Result<Vec<Vec<FloatType>>> {
    if data.parts_count() > 0 {
        data.parts()[..data.parts_count()]
            .iter()
            .map(onsets)
            .collect()
    } else {
        Ok(vec![onsets(data)?])
    }
}

fn note_density(data: &DataInstance, vector: &mut [Value]) -> Result<()> {
    let seconds = data.seconds_map()?;
    let last = seconds
        .iter()
        .map(|note| note.end)
        .fold(None, |most: Option<FloatType>, end| {
            Some(most.map_or(end, |most| most.max(end)))
        });
    vector[0] = match last {
        Some(0.0) => return Err(Error::Feature("float division by zero".to_string())),
        Some(last) => (seconds.len() as FloatType / last).into(),
        None => Value::Float(0.0),
    };
    Ok(())
}

fn average_note_duration(data: &DataInstance, vector: &mut [Value]) -> Result<()> {
    let seconds = data.seconds_map()?;
    if seconds.is_empty() {
        return Err(lacks_notes());
    }
    let mut total = 0.0;
    for note in seconds {
        total += note.duration;
    }
    vector[0] = (total / seconds.len() as FloatType).into();
    Ok(())
}

fn variability_of_note_duration(data: &DataInstance, vector: &mut [Value]) -> Result<()> {
    let seconds = data.seconds_map()?;
    if seconds.is_empty() {
        return Err(lacks_notes());
    }
    let durations: Vec<FloatType> = seconds.iter().map(|note| note.duration).collect();
    vector[0] = crate::statistics::pstdev(&durations).into();
    Ok(())
}

fn maximum_note_duration(data: &DataInstance, vector: &mut [Value]) -> Result<()> {
    let seconds = data.seconds_map()?;
    if seconds.is_empty() {
        return Err(lacks_notes());
    }
    let mut longest = 0.0;
    for note in seconds {
        if note.duration > longest {
            longest = note.duration;
        }
    }
    vector[0] = longest.into();
    Ok(())
}

fn minimum_note_duration(data: &DataInstance, vector: &mut [Value]) -> Result<()> {
    let seconds = data.seconds_map()?;
    let Some(first) = seconds.first() else {
        return Err(lacks_notes());
    };
    let mut shortest = first.duration;
    for note in seconds {
        if note.duration < shortest {
            shortest = note.duration;
        }
    }
    vector[0] = shortest.into();
    Ok(())
}

fn staccato_incidence(data: &DataInstance, vector: &mut [Value]) -> Result<()> {
    let seconds = data.seconds_map()?;
    if seconds.is_empty() {
        return Err(lacks_notes());
    }
    let short = seconds.iter().filter(|note| note.duration < 0.10).count();
    vector[0] = (short as FloatType / seconds.len() as FloatType).into();
    Ok(())
}

fn average_time_between_attacks(data: &DataInstance, vector: &mut [Value]) -> Result<()> {
    let onsets = onsets(data)?;
    if onsets.is_empty() {
        return Err(lacks_notes());
    }
    let gaps = time_between(&onsets);
    if gaps.is_empty() {
        return Err(Error::Feature("division by zero".to_string()));
    }
    vector[0] = (crate::statistics::python_sum(&gaps) / gaps.len() as FloatType).into();
    Ok(())
}

fn variability_of_time_between_attacks(data: &DataInstance, vector: &mut [Value]) -> Result<()> {
    let onsets = onsets(data)?;
    if onsets.is_empty() {
        return Err(lacks_notes());
    }
    let gaps = time_between(&onsets);
    if gaps.is_empty() {
        return Err(Error::Feature(
            "pstdev requires at least one data point".to_string(),
        ));
    }
    vector[0] = crate::statistics::pstdev(&gaps).into();
    Ok(())
}

fn average_time_between_attacks_for_each_voice(
    data: &DataInstance,
    vector: &mut [Value],
) -> Result<()> {
    let mut averages = Vec::new();
    for onsets in onsets_by_part(data)? {
        let gaps = time_between(&onsets);
        if gaps.is_empty() {
            return Err(Error::Feature("at least one part lacks notes".to_string()));
        }
        averages.push(crate::statistics::python_sum(&gaps) / gaps.len() as FloatType);
    }
    vector[0] = (crate::statistics::python_sum(&averages) / averages.len() as FloatType).into();
    Ok(())
}

fn average_variability_of_time_between_attacks_for_each_voice(
    data: &DataInstance,
    vector: &mut [Value],
) -> Result<()> {
    let mut deviations = Vec::new();
    for onsets in onsets_by_part(data)? {
        let gaps = time_between(&onsets);
        if gaps.is_empty() {
            return Err(Error::Feature("at least one part lacks notes".to_string()));
        }
        deviations.push(crate::statistics::pstdev(&gaps));
    }
    vector[0] = (crate::statistics::python_sum(&deviations) / deviations.len() as FloatType).into();
    Ok(())
}

fn initial_tempo(data: &DataInstance, vector: &mut [Value]) -> Result<()> {
    let boundaries = data.metronome_mark_boundaries()?;
    vector[0] = boundaries
        .first()
        .and_then(|(_, _, mark)| mark.sounding_quarter_bpm())
        .ok_or_else(|| Error::Feature("the first tempo says no number".to_string()))?
        .into();
    Ok(())
}

fn initial_time_signature(data: &DataInstance, vector: &mut [Value]) -> Result<()> {
    if let Some(meter) = data.time_signatures().first() {
        vector[0] = meter.numerator().into();
        vector[1] = meter.denominator().into();
    }
    Ok(())
}

fn compound_or_simple_meter(data: &DataInstance, vector: &mut [Value]) -> Result<()> {
    if data
        .time_signatures()
        .first()
        .is_some_and(|meter| meter.beat_division_count_name() == "Compound")
    {
        vector[0] = Value::Integer(1);
    }
    Ok(())
}

fn triple_meter(data: &DataInstance, vector: &mut [Value]) -> Result<()> {
    if data
        .time_signatures()
        .first()
        .is_some_and(|meter| meter.numerator() == 3)
    {
        vector[0] = Value::Integer(1);
    }
    Ok(())
}

fn quintuple_meter(data: &DataInstance, vector: &mut [Value]) -> Result<()> {
    if data
        .time_signatures()
        .first()
        .is_some_and(|meter| meter.numerator() == 5)
    {
        vector[0] = Value::Integer(1);
    }
    Ok(())
}

fn changes_of_meter(data: &DataInstance, vector: &mut [Value]) -> Result<()> {
    let meters = data.time_signatures();
    if let [first, rest @ ..] = meters.as_slice()
        && rest.iter().any(|meter| !first.ratio_equal(meter))
    {
        vector[0] = Value::Integer(1);
    }
    Ok(())
}

fn duration(data: &DataInstance, vector: &mut [Value]) -> Result<()> {
    let seconds = data.seconds_map()?;
    if seconds.is_empty() {
        return Err(Error::Feature("input lacks duration".to_string()));
    }
    vector[0] = seconds
        .iter()
        .map(|note| note.end)
        .fold(FloatType::NEG_INFINITY, FloatType::max)
        .into();
    Ok(())
}

fn maximum_number_of_independent_voices(data: &DataInstance, vector: &mut [Value]) -> Result<()> {
    vector[0] = data
        .parts_sounding()?
        .iter()
        .copied()
        .max()
        .unwrap_or(0)
        .into();
    Ok(())
}

fn average_number_of_independent_voices(data: &DataInstance, vector: &mut [Value]) -> Result<()> {
    let sounding = data.parts_sounding()?;
    if sounding.is_empty() {
        return Err(lacks_notes());
    }
    vector[0] = (sounding.iter().sum::<usize>() as FloatType / sounding.len() as FloatType).into();
    Ok(())
}

fn variability_of_number_of_independent_voices(
    data: &DataInstance,
    vector: &mut [Value],
) -> Result<()> {
    let sounding = data.parts_sounding()?;
    if sounding.is_empty() {
        return Err(lacks_notes());
    }
    let counts: Vec<FloatType> = sounding.iter().map(|count| *count as FloatType).collect();
    vector[0] = crate::statistics::pstdev(&counts).into();
    Ok(())
}

/// Each part of the piece partitioned by instrument, as the program of its
/// first instrument (`None` where it states none) and how many notes it
/// holds: what music21's instrument features read off
/// `partitionByInstrument`. Nothing where the partition is empty.
fn instrument_parts(data: &DataInstance) -> Result<Vec<(Option<Option<u8>>, usize)>> {
    let partitioned = crate::instrument::partition_by_instrument(data.prepared());
    if partitioned.is_empty() {
        return Err(Error::Feature("input lacks instruments".to_string()));
    }
    Ok(partitioned
        .parts()
        .into_iter()
        .map(|part| {
            let program = part
                .events()
                .iter()
                .find_map(|event| match event.element() {
                    StreamElement::Instrument(instrument) => Some(instrument.midi_program()),
                    _ => None,
                });
            let notes = part
                .recurse()
                .into_iter()
                .filter(|(_, element)| {
                    matches!(
                        element,
                        StreamElement::Note(_)
                            | StreamElement::Chord(_)
                            | StreamElement::Unpitched(_)
                            | StreamElement::PercussionChord(_)
                            | StreamElement::ChordSymbol(_)
                    )
                })
                .count();
            (program, notes)
        })
        .collect())
}

/// The program of a part's first instrument, refused where it has none or
/// the instrument names no program, as music21 refuses it.
fn program_of(program: Option<Option<u8>>) -> Result<usize> {
    match program {
        Some(Some(program)) => Ok(usize::from(program)),
        Some(None) => Err(Error::Feature(
            "an instrument lacks a midiProgram".to_string(),
        )),
        None => Err(Error::Feature(
            "'NoneType' object has no attribute 'midiProgram'".to_string(),
        )),
    }
}

/// How many pitches sound in the piece, which the instrument features
/// divide by.
fn pitch_total(data: &DataInstance) -> usize {
    data.pitch_class_histogram().iter().sum()
}

fn pitched_instruments_present(data: &DataInstance, vector: &mut [Value]) -> Result<()> {
    for (program, notes) in instrument_parts(data)? {
        if notes > 0 {
            vector[program_of(program)?] = Value::Integer(1);
        }
    }
    Ok(())
}

fn note_prevalence_of_pitched_instruments(data: &DataInstance, vector: &mut [Value]) -> Result<()> {
    let total = pitch_total(data);
    for (program, notes) in instrument_parts(data)? {
        if notes > 0 {
            let program = program_of(program)?;
            if total == 0 {
                return Err(Error::Feature("division by zero".to_string()));
            }
            vector[program] = (notes as FloatType / total as FloatType).into();
        }
    }
    Ok(())
}

fn variability_of_note_prevalence_of_pitched_instruments(
    data: &DataInstance,
    vector: &mut [Value],
) -> Result<()> {
    let parts = instrument_parts(data)?;
    let total = pitch_total(data);
    if total == 0 {
        return Err(lacks_notes());
    }
    let shares: Vec<FloatType> = parts
        .iter()
        .filter(|(_, notes)| *notes > 0)
        .map(|(_, notes)| *notes as FloatType / total as FloatType)
        .collect();
    if shares.is_empty() {
        return Err(Error::Feature("division by zero".to_string()));
    }
    let mean = crate::statistics::python_sum(&shares) / shares.len() as FloatType;
    let squares: Vec<FloatType> = shares.iter().map(|share| (share - mean).powi(2)).collect();
    vector[0] = (crate::statistics::python_sum(&squares) / squares.len() as FloatType)
        .sqrt()
        .into();
    Ok(())
}

fn number_of_pitched_instruments(data: &DataInstance, vector: &mut [Value]) -> Result<()> {
    vector[0] = instrument_parts(data)?
        .iter()
        .filter(|(_, notes)| *notes > 0)
        .count()
        .into();
    Ok(())
}

/// The share of the piece's pitches played by instruments of these
/// programs: music21's `InstrumentFractionFeature`.
fn instrument_fraction(data: &DataInstance, programs: &[usize]) -> Result<FloatType> {
    let parts = instrument_parts(data)?;
    let total = pitch_total(data);
    if total == 0 {
        return Err(lacks_notes());
    }
    let mut count = 0;
    for (program, notes) in parts {
        // An instrument naming no program is in no family; a part with no
        // instrument at all is refused.
        let Some(program) = program else {
            return Err(program_of(None).unwrap_err());
        };
        if program.is_some_and(|program| programs.contains(&usize::from(program))) {
            count += notes;
        }
    }
    Ok(count as FloatType / total as FloatType)
}

fn string_keyboard_fraction(data: &DataInstance, vector: &mut [Value]) -> Result<()> {
    vector[0] = instrument_fraction(data, &[0, 1, 2, 3, 4, 5, 6, 7])?.into();
    Ok(())
}

fn acoustic_guitar_fraction(data: &DataInstance, vector: &mut [Value]) -> Result<()> {
    vector[0] = instrument_fraction(data, &[24, 25])?.into();
    Ok(())
}

fn electric_guitar_fraction(data: &DataInstance, vector: &mut [Value]) -> Result<()> {
    vector[0] = instrument_fraction(data, &[26, 27, 28, 29, 30, 31])?.into();
    Ok(())
}

fn violin_fraction(data: &DataInstance, vector: &mut [Value]) -> Result<()> {
    vector[0] = instrument_fraction(data, &[40, 110])?.into();
    Ok(())
}

fn saxophone_fraction(data: &DataInstance, vector: &mut [Value]) -> Result<()> {
    vector[0] = instrument_fraction(data, &[64, 65, 66, 67])?.into();
    Ok(())
}

fn brass_fraction(data: &DataInstance, vector: &mut [Value]) -> Result<()> {
    vector[0] = instrument_fraction(data, &[56, 57, 58, 59, 60, 61])?.into();
    Ok(())
}

fn woodwinds_fraction(data: &DataInstance, vector: &mut [Value]) -> Result<()> {
    vector[0] =
        instrument_fraction(data, &[68, 69, 70, 71, 72, 73, 74, 75, 76, 77, 78, 79])?.into();
    Ok(())
}

fn orchestral_strings_fraction(data: &DataInstance, vector: &mut [Value]) -> Result<()> {
    vector[0] = instrument_fraction(data, &[41, 42, 43, 44, 45])?.into();
    Ok(())
}

fn string_ensemble_fraction(data: &DataInstance, vector: &mut [Value]) -> Result<()> {
    vector[0] = instrument_fraction(data, &[48, 49, 50, 51])?.into();
    Ok(())
}

fn electric_instrument_fraction(data: &DataInstance, vector: &mut [Value]) -> Result<()> {
    vector[0] = instrument_fraction(
        data,
        &[
            4, 5, 16, 18, 26, 27, 28, 29, 30, 31, 33, 34, 35, 36, 37, 38, 39,
        ],
    )?
    .into();
    Ok(())
}
