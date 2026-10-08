//! The features music21 adds of its own: music21's `features.native`. Each
//! is an [`Extractor`] in [`NATIVE`], found by its id with [`extractor`].
//!
//! The key features read the keys the piece states or, failing those, the
//! key it is analysed to be in; the quarter-length features (`QL`) read how
//! long its notes are; the simultaneity features (`CS`) read the chords of
//! the piece chordified, and its chord symbols; the Landini cadence (`MC1`)
//! reads how each part's melody ends. music21's habits are kept where they
//! decide a value: the most common length is the last of those equally
//! common, and a chord symbol that sounds nothing refuses the bass-motion
//! feature, as music21 finds no bass in it.
//!
//! music21's language feature (`TX1`), which reads lyrics against texts in
//! seven languages, is not here.

use super::{DataInstance, Extractor, Value};
use crate::{
    chord::Chord,
    chordsymbol::ChordSymbol,
    defaults::{FloatType, IntegerType},
    error::{Error, Result},
    pitch::Pitch,
    stream::StreamElement,
};

/// Every native extractor music21 lists, in music21's order, but its
/// language feature.
pub const NATIVE: &[Extractor] = &[
    Extractor {
        id: "P22",
        name: "Quality",
        description: "Set to 0 if the Key or KeySignature indicates that a recording is major, set to 1 if it indicates that it is minor. Music21 addition: if no key mode is found in the piece, or conflicting modes in the keys, analyze the piece to discover what mode it is most likely in.",
        dimensions: 1,
        discrete: true,
        normalize: false,
        process: quality,
    },
    Extractor {
        id: "K1",
        name: "Tonal Certainty",
        description: "A floating point magnitude value that suggest tonal certainty based on automatic key analysis.",
        dimensions: 1,
        discrete: false,
        normalize: false,
        process: tonal_certainty,
    },
    Extractor {
        id: "QL1",
        name: "Unique Note Quarter Lengths",
        description: "The number of unique note quarter lengths.",
        dimensions: 1,
        discrete: true,
        normalize: false,
        process: unique_note_quarter_lengths,
    },
    Extractor {
        id: "QL2",
        name: "Most Common Note Quarter Length",
        description: "The value of the most common quarter length.",
        dimensions: 1,
        discrete: false,
        normalize: false,
        process: most_common_note_quarter_length,
    },
    Extractor {
        id: "QL3",
        name: "Most Common Note Quarter Length Prevalence",
        description: "Fraction of notes that have the most common quarter length.",
        dimensions: 1,
        discrete: false,
        normalize: false,
        process: most_common_note_quarter_length_prevalence,
    },
    Extractor {
        id: "QL4",
        name: "Range of Note Quarter Lengths",
        description: "Difference between the longest and shortest quarter lengths.",
        dimensions: 1,
        discrete: false,
        normalize: false,
        process: range_of_note_quarter_lengths,
    },
    Extractor {
        id: "CS1",
        name: "Unique Pitch Class Set Simultaneities",
        description: "Number of unique pitch class simultaneities.",
        dimensions: 1,
        discrete: false,
        normalize: false,
        process: unique_pitch_class_set_simultaneities,
    },
    Extractor {
        id: "CS2",
        name: "Unique Set Class Simultaneities",
        description: "Number of unique set class simultaneities.",
        dimensions: 1,
        discrete: false,
        normalize: false,
        process: unique_set_class_simultaneities,
    },
    Extractor {
        id: "CS3",
        name: "Most Common Pitch Class Set Simultaneity Prevalence",
        description: "Fraction of all pitch class simultaneities that are the most common simultaneity.",
        dimensions: 1,
        discrete: false,
        normalize: false,
        process: most_common_pitch_class_set_simultaneity_prevalence,
    },
    Extractor {
        id: "CS4",
        name: "Most Common Set Class Simultaneity Prevalence",
        description: "Fraction of all set class simultaneities that are the most common simultaneity.",
        dimensions: 1,
        discrete: false,
        normalize: false,
        process: most_common_set_class_simultaneity_prevalence,
    },
    Extractor {
        id: "CS5",
        name: "Major Triad Simultaneity Prevalence",
        description: "Percentage of all simultaneities that are major triads.",
        dimensions: 1,
        discrete: false,
        normalize: false,
        process: major_triad_simultaneity_prevalence,
    },
    Extractor {
        id: "CS6",
        name: "Minor Triad Simultaneity Prevalence",
        description: "Percentage of all simultaneities that are minor triads.",
        dimensions: 1,
        discrete: false,
        normalize: false,
        process: minor_triad_simultaneity_prevalence,
    },
    Extractor {
        id: "CS7",
        name: "Dominant Seventh Simultaneity Prevalence",
        description: "Percentage of all simultaneities that are dominant seventh.",
        dimensions: 1,
        discrete: false,
        normalize: false,
        process: dominant_seventh_simultaneity_prevalence,
    },
    Extractor {
        id: "CS8",
        name: "Diminished Triad Simultaneity Prevalence",
        description: "Percentage of all simultaneities that are diminished triads.",
        dimensions: 1,
        discrete: false,
        normalize: false,
        process: diminished_triad_simultaneity_prevalence,
    },
    Extractor {
        id: "CS9",
        name: "Triad Simultaneity Prevalence",
        description: "Proportion of all simultaneities that form triads.",
        dimensions: 1,
        discrete: false,
        normalize: false,
        process: triad_simultaneity_prevalence,
    },
    Extractor {
        id: "CS10",
        name: "Diminished Seventh Simultaneity Prevalence",
        description: "Percentage of all simultaneities that are diminished seventh chords.",
        dimensions: 1,
        discrete: false,
        normalize: false,
        process: diminished_seventh_simultaneity_prevalence,
    },
    Extractor {
        id: "CS11",
        name: "Incorrectly Spelled Triad Prevalence",
        description: "Percentage of all triads that are spelled incorrectly.",
        dimensions: 1,
        discrete: false,
        normalize: false,
        process: incorrectly_spelled_triad_prevalence,
    },
    Extractor {
        id: "CS12",
        name: "Chord Bass Motion",
        description: "12-element vector showing the fraction of chords that move by x semitones (where x=0 is always 0 unless there are 0 or 1 harmonies, in which case it is 1).",
        dimensions: 12,
        discrete: false,
        normalize: false,
        process: chord_bass_motion,
    },
    Extractor {
        id: "MC1",
        name: "Ends With Landini Melodic Contour",
        description: "Boolean that indicates the presence of a Landini-like cadential figure in one or more parts.",
        dimensions: 1,
        discrete: false,
        normalize: false,
        process: landini_cadence,
    },
];

/// The native extractor with this id, such as `QL1` or `CS12`.
pub fn extractor(id: &str) -> Option<&'static Extractor> {
    NATIVE.iter().find(|extractor| extractor.id == id)
}

fn lacks_notes() -> Error {
    Error::Feature("input lacks notes".to_string())
}

/// music21's `pitchClass`: the pitch space rounded half to even, folded
/// into the octave.
fn pitch_class(pitch: &Pitch) -> usize {
    (pitch.ps().round_ties_even() as i64).rem_euclid(12) as usize
}

fn mode_value(mode: &str) -> Option<Value> {
    match mode {
        "major" => Some(Value::Integer(0)),
        "minor" => Some(Value::Integer(1)),
        _ => None,
    }
}

fn quality(data: &DataInstance, vector: &mut [Value]) -> Result<()> {
    // Several stated keys are taken where they agree in mode, as the keys
    // of transposing parts do.
    let modes = data.key_modes();
    if let Some((first, rest)) = modes.split_first()
        && rest.iter().all(|mode| mode == first)
        && let Some(value) = mode_value(first)
    {
        vector[0] = value;
        return Ok(());
    }
    let mode = data
        .analyzed_key()
        .and_then(<[_]>::first)
        .ok_or_else(|| {
            Error::Feature("failed to get likely keys for Stream component".to_string())
        })?
        .key()
        .mode()
        .to_string();
    vector[0] = mode_value(&mode).ok_or_else(|| {
        Error::Feature(
            "should be able to get a mode from something here -- perhaps there are no notes?"
                .to_string(),
        )
    })?;
    Ok(())
}

fn tonal_certainty(data: &DataInstance, vector: &mut [Value]) -> Result<()> {
    let estimates = data.analyzed_key().ok_or_else(|| {
        Error::Feature("failed to get likely keys for Stream component".to_string())
    })?;
    vector[0] = crate::analysis::tonal_certainty(estimates).into();
    Ok(())
}

fn unique_note_quarter_lengths(data: &DataInstance, vector: &mut [Value]) -> Result<()> {
    vector[0] = data.quarter_length_histogram().len().into();
    Ok(())
}

fn most_common_note_quarter_length(data: &DataInstance, vector: &mut [Value]) -> Result<()> {
    let mut maximum = 0;
    let mut most_common = Value::Integer(0);
    for &(length, count) in data.quarter_length_histogram() {
        if count >= maximum {
            maximum = count;
            most_common = length.into();
        }
    }
    vector[0] = most_common;
    Ok(())
}

/// The share of a histogram's counts its largest count has.
fn prevalence<'a>(counts: impl Iterator<Item = &'a usize>) -> FloatType {
    let (total, largest) = counts.fold((0, 0), |(total, largest), &count| {
        (total + count, largest.max(count))
    });
    if total == 0 {
        0.0
    } else {
        largest as FloatType / total as FloatType
    }
}

fn most_common_note_quarter_length_prevalence(
    data: &DataInstance,
    vector: &mut [Value],
) -> Result<()> {
    let histogram = data.quarter_length_histogram();
    if histogram.is_empty() {
        return Err(lacks_notes());
    }
    vector[0] = prevalence(histogram.iter().map(|(_, count)| count)).into();
    Ok(())
}

fn range_of_note_quarter_lengths(data: &DataInstance, vector: &mut [Value]) -> Result<()> {
    let lengths = data
        .quarter_length_histogram()
        .iter()
        .map(|(length, _)| *length);
    let shortest = lengths
        .clone()
        .reduce(FloatType::min)
        .ok_or_else(lacks_notes)?;
    let longest = lengths.reduce(FloatType::max).ok_or_else(lacks_notes)?;
    vector[0] = (longest - shortest).into();
    Ok(())
}

/// How often each label comes among the chords of the piece chordified, in
/// the order each is first met: a `Counter` music21 keeps of them.
fn chord_histogram(
    data: &DataInstance,
    label: impl Fn(&Chord) -> String,
) -> Result<Vec<(String, usize)>> {
    let mut counts: Vec<(String, usize)> = Vec::new();
    for chord in data.chordified_chords()? {
        let label = label(chord);
        match counts.iter_mut().find(|(known, _)| *known == label) {
            Some((_, count)) => *count += 1,
            None => counts.push((label, 1)),
        }
    }
    Ok(counts)
}

fn pitch_class_set_histogram(data: &DataInstance) -> Result<Vec<(String, usize)>> {
    chord_histogram(data, Chord::ordered_pitch_classes_string)
}

/// The set classes of the chords chordified, music21's `forteClassTnI`,
/// which is `N/A` for a chord that sounds nothing.
fn set_class_histogram(data: &DataInstance) -> Result<Vec<(String, usize)>> {
    chord_histogram(data, |chord| {
        chord.forte_class_tni().unwrap_or_else(|| "N/A".to_string())
    })
}

fn unique_pitch_class_set_simultaneities(data: &DataInstance, vector: &mut [Value]) -> Result<()> {
    vector[0] = pitch_class_set_histogram(data)?.len().into();
    Ok(())
}

fn unique_set_class_simultaneities(data: &DataInstance, vector: &mut [Value]) -> Result<()> {
    vector[0] = set_class_histogram(data)?.len().into();
    Ok(())
}

fn most_common_pitch_class_set_simultaneity_prevalence(
    data: &DataInstance,
    vector: &mut [Value],
) -> Result<()> {
    let histogram = pitch_class_set_histogram(data)?;
    if histogram.is_empty() {
        return Err(lacks_notes());
    }
    vector[0] = prevalence(histogram.iter().map(|(_, count)| count)).into();
    Ok(())
}

fn most_common_set_class_simultaneity_prevalence(
    data: &DataInstance,
    vector: &mut [Value],
) -> Result<()> {
    let histogram = set_class_histogram(data)?;
    if histogram.is_empty() {
        return Err(lacks_notes());
    }
    vector[0] = prevalence(histogram.iter().map(|(_, count)| count)).into();
    Ok(())
}

/// Some of music21's `typesHistogram` counts over the chords chordified,
/// as a share of how many chords there are, nought where there are none.
fn share_of_chords(
    data: &DataInstance,
    vector: &mut [Value],
    kinds: &[fn(&Chord) -> bool],
) -> Result<()> {
    let chords = data.chordified_chords()?;
    let counted: usize = kinds
        .iter()
        .map(|kind| chords.iter().filter(|chord| kind(chord)).count())
        .sum();
    vector[0] = if chords.is_empty() {
        Value::Integer(0)
    } else {
        (counted as FloatType / chords.len() as FloatType).into()
    };
    Ok(())
}

fn major_triad_simultaneity_prevalence(data: &DataInstance, vector: &mut [Value]) -> Result<()> {
    share_of_chords(
        data,
        vector,
        &[Chord::is_major_triad, Chord::is_incomplete_major_triad],
    )
}

fn minor_triad_simultaneity_prevalence(data: &DataInstance, vector: &mut [Value]) -> Result<()> {
    share_of_chords(
        data,
        vector,
        &[Chord::is_minor_triad, Chord::is_incomplete_minor_triad],
    )
}

fn dominant_seventh_simultaneity_prevalence(
    data: &DataInstance,
    vector: &mut [Value],
) -> Result<()> {
    share_of_chords(data, vector, &[Chord::is_dominant_seventh])
}

fn diminished_triad_simultaneity_prevalence(
    data: &DataInstance,
    vector: &mut [Value],
) -> Result<()> {
    share_of_chords(data, vector, &[Chord::is_diminished_triad])
}

fn triad_simultaneity_prevalence(data: &DataInstance, vector: &mut [Value]) -> Result<()> {
    share_of_chords(data, vector, &[Chord::is_triad])
}

fn diminished_seventh_simultaneity_prevalence(
    data: &DataInstance,
    vector: &mut [Value],
) -> Result<()> {
    share_of_chords(data, vector, &[Chord::is_diminished_seventh])
}

/// The share of the chords whose set class is a triad's -- major, minor,
/// diminished or augmented -- that are not spelled as a triad.
fn incorrectly_spelled_triad_prevalence(data: &DataInstance, vector: &mut [Value]) -> Result<()> {
    let chords = data.chordified_chords()?;
    if chords.is_empty() {
        return Err(lacks_notes());
    }
    let spelled = chords.iter().filter(|chord| chord.is_triad()).count() as FloatType;
    let set_classes = set_class_histogram(data)?;
    let triads: usize = set_classes
        .iter()
        .filter(|(class, _)| matches!(class.as_str(), "3-10" | "3-11" | "3-12"))
        .map(|(_, count)| count)
        .sum();
    if triads == 0 {
        return Err(Error::Feature("input lacks Forte triads".to_string()));
    }
    let triads = triads as FloatType;
    vector[0] = ((triads - spelled) / triads).into();
    Ok(())
}

/// The pitch class a chord symbol's bass is on, its root where it names
/// no other bass.
fn bass_class(symbol: &ChordSymbol) -> usize {
    pitch_class(symbol.bass().unwrap_or(symbol.root()))
}

/// How far the bass moves down, in semitones folded into the octave, from
/// each chord symbol to the next that has another bass, each as a share of
/// all such moves; the first value, a bass staying put, is one where the
/// bass never moves. A symbol that sounds nothing is passed over.
fn chord_bass_motion(data: &DataInstance, vector: &mut [Value]) -> Result<()> {
    let flat = data.prepared().flatten();
    let mut motion = [0_usize; 12];
    let mut total = 0;
    let mut last: Option<&ChordSymbol> = None;
    for event in flat.events() {
        let StreamElement::ChordSymbol(this) = event.element() else {
            continue;
        };
        if this.is_no_chord() {
            continue;
        }
        let Some(previous) = last else {
            last = Some(this);
            continue;
        };
        let last_bass = bass_class(previous);
        let this_bass = bass_class(this);
        if last_bass != this_bass {
            motion[(last_bass + 12 - this_bass) % 12] += 1;
            total += 1;
            last = Some(this);
        }
    }
    // music21 writes every value as a float here, the ones it never
    // reaches too.
    for value in vector.iter_mut() {
        *value = Value::Float(0.0);
    }
    if total == 0 {
        vector[0] = Value::Float(1.0);
    } else {
        for (value, count) in vector.iter_mut().zip(motion).skip(1) {
            *value = (count as FloatType / total as FloatType).into();
        }
    }
    Ok(())
}

/// Whether any part's melody, its repeated notes left out, ends falling a
/// tone and rising a minor third, or falling a semitone and a tone and
/// rising a minor third.
fn landini_cadence(data: &DataInstance, vector: &mut [Value]) -> Result<()> {
    const ENDINGS: [&[IntegerType]; 2] = [&[-2, 3], &[-1, -2, 3]];
    let contours: Vec<&[IntegerType]> = if data.parts_count() > 0 {
        data.parts()[..data.parts_count()]
            .iter()
            .map(DataInstance::contour_list)
            .collect()
    } else {
        vec![data.contour_list()]
    };
    let found = contours.iter().any(|contour| {
        let moving: Vec<IntegerType> = contour.iter().copied().filter(|step| *step != 0).collect();
        ENDINGS.iter().any(|ending| moving.ends_with(ending))
    });
    if found {
        vector[0] = Value::Integer(1);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stream::Stream;
    use crate::tinynotation::from_tiny_notation;

    fn vector(id: &str, data: &DataInstance) -> Vec<FloatType> {
        extractor(id)
            .expect("a native extractor")
            .extract(data)
            .expect("a feature")
            .vector()
            .to_vec()
    }

    fn symbols(figures: &[&str]) -> Stream {
        let mut stream = Stream::new();
        for (offset, figure) in figures.iter().enumerate() {
            let symbol = ChordSymbol::parse_music21(*figure).expect("a chord symbol");
            stream.insert(offset as FloatType, StreamElement::ChordSymbol(symbol));
        }
        stream
    }

    #[test]
    fn the_bass_motion_is_read_from_chord_symbols() -> Result<()> {
        // Each read off music21's ChordBassMotionFeature.
        let data = DataInstance::new(&symbols(&["C", "F", "G", "C"]))?;
        let mut expected = vec![0.0; 12];
        expected[7] = 2.0 / 3.0;
        expected[10] = 1.0 / 3.0;
        assert_eq!(vector("CS12", &data), expected);

        // A symbol that sounds nothing is passed over, wherever it stands.
        let mut silent = symbols(&["C", "G"]);
        silent.insert(0.5, StreamElement::ChordSymbol(ChordSymbol::no_chord(None)));
        silent.insert(2.0, StreamElement::ChordSymbol(ChordSymbol::no_chord(None)));
        let data = DataInstance::new(&silent)?;
        let mut expected = vec![0.0; 12];
        expected[5] = 1.0;
        assert_eq!(vector("CS12", &data), expected);
        let mut alone = symbols(&["C"]);
        alone.insert(1.0, StreamElement::ChordSymbol(ChordSymbol::no_chord(None)));
        let data = DataInstance::new(&alone)?;
        let mut still = vec![0.0; 12];
        still[0] = 1.0;
        assert_eq!(vector("CS12", &data), still);
        Ok(())
    }

    #[test]
    fn a_landini_cadence_is_found_at_the_end_of_a_melody() -> Result<()> {
        let cadence = DataInstance::new(&from_tiny_notation("4/4 c'4 b a c'")?)?;
        assert_eq!(vector("MC1", &cadence), [1.0]);
        let plain = DataInstance::new(&from_tiny_notation("4/4 c'4 b c' c'")?)?;
        assert_eq!(vector("MC1", &plain), [0.0]);
        Ok(())
    }

    #[test]
    fn note_lengths_and_the_key_are_read_as_music21_reads_them() -> Result<()> {
        // Each read off music21's native extractors.
        let data = DataInstance::new(&from_tiny_notation("4/4 c4 d8 e8 f2")?)?;
        assert_eq!(vector("QL1", &data), [3.0]);
        assert_eq!(vector("QL2", &data), [0.5]);
        assert_eq!(vector("QL3", &data), [0.5]);
        assert_eq!(vector("QL4", &data), [1.5]);
        assert_eq!(vector("CS1", &data), [0.0]);
        assert_eq!(vector("P22", &data), [0.0]);
        let certainty = vector("K1", &data)[0];
        assert!((certainty - 0.788_866_872_590_800_5).abs() < 1e-12);
        Ok(())
    }
}
