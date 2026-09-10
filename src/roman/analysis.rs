//! Naming a chord as a numeral in a key, and the figure tuples music21
//! reads a chord's pitches as above its bass.

use super::*;

impl RomanNumeral {
    /// Performs functional Roman-numeral analysis in a key.
    pub fn analyze(chord: &Chord, key: Key) -> Result<Option<Self>> {
        let Some(root_name) = chord.root_pitch_name() else {
            return Ok(None);
        };
        let root = Pitch::from_name(normalize_pitch_name(&root_name))?;
        Self::analyze_with_root(chord, key, &root)
    }

    /// Performs Roman-numeral analysis using an explicit harmonic root.
    ///
    /// This is useful for pitch-class-set browser views where the caller has
    /// already chosen a transposition root and does not want inversion or root
    /// inference to pick a different chord member.
    pub fn analyze_with_root(chord: &Chord, key: Key, root: &Pitch) -> Result<Option<Self>> {
        if let Some(kind) = augmented_sixth_kind_for_key(chord, &key)? {
            return Self::new(kind.figure(), key).map(Some);
        }

        let root_pc = pitch_class(root);
        let intervals = intervals_above_root(chord, root_pc);
        if !intervals.contains(&0) {
            return Ok(None);
        }

        let Some((degree, accidental)) = degree_for_root(&key, root)? else {
            return Ok(None);
        };

        let symbol = chord
            .chord_symbols_with_root(root_pc)?
            .into_iter()
            .find_map(|figure| ChordSymbol::parse(figure).ok());
        let quality = symbol
            .as_ref()
            .map(symbol_quality)
            .unwrap_or_else(|| quality_from_intervals(&intervals));

        let figure = roman_figure(
            degree,
            accidental,
            quality,
            symbol.as_ref(),
            &intervals,
            roman_inversion(chord),
        );

        Self::new(figure, key).map(Some)
    }
}

/// The figured-bass suffix a Roman numeral takes for a chord's inversion:
/// music21's `romanInversionName`, `6` and `64` for a triad, `7`, `65`, `43`
/// and `42` for a seventh chord, and nothing otherwise. The inversion is the
/// chord's own unless one is given.
pub fn roman_inversion_name(chord: &Chord, inversion: Option<u8>) -> String {
    match chord.root() {
        Some(root) => inversion_name_from_root(chord, root, inversion),
        None => String::new(),
    }
}

pub(super) fn inversion_name_from_root(
    chord: &Chord,
    root: &Pitch,
    inversion: Option<u8>,
) -> String {
    let Some(inversion) = inversion.or_else(|| chord.inversion_with_root(root)) else {
        return String::new();
    };
    let has = |step: u8| chord.chord_step_from(step, root).is_some();
    let suffix = if has(7) {
        match inversion {
            0 => "7",
            1 => "65",
            2 => "43",
            3 => "42",
            _ => "",
        }
    } else if has(1) && has(3) {
        match inversion {
            1 => "6",
            2 => "64",
            _ => "",
        }
    } else {
        ""
    };
    suffix.to_string()
}

impl FigureTuple {
    /// The figure of `pitch` above `reference` in `key`: music21's
    /// `FigureTuple.fromPitchAndReference`. An `A-3` above an `F#2` bass in C
    /// major is a third above the bass and a semitone below the `A` C major
    /// has, so it is `3`, altered by `-1`, written `b`. A minor key is read
    /// as the natural minor.
    pub fn from_pitch_and_reference(pitch: &Pitch, key: &Key, reference: &Pitch) -> Result<Self> {
        let (_, accidental) = key.as_scale()?.degree_and_accidental_of(pitch)?;
        let degree = Interval::between_pitches(reference, pitch)?
            .generic()
            .mod7();
        let alter = accidental.map_or(0.0, |accidental| accidental.alter());
        Ok(Self {
            deg_from_ref_pitch: u8::try_from(degree).unwrap_or(1),
            alter,
            prefix: figure_prefix(alter as IntegerType),
        })
    }
}

/// The accidental written in front of a figure for an alteration: sharps
/// for a positive one, flats for a negative one.
pub(super) fn figure_prefix(alter: IntegerType) -> String {
    match alter {
        0 => String::new(),
        sharps if sharps > 0 => "#".repeat(sharps as usize),
        flats => "b".repeat(flats.unsigned_abs() as usize),
    }
}

/// One figure per pitch of the chord, each read above the chord's bass:
/// music21's `figureTuples`.
pub fn figure_tuples(chord: &Chord, key: &Key) -> Result<Vec<PitchFigureTuple>> {
    let Some(bass) = chord.bass() else {
        return Ok(Vec::new());
    };
    chord
        .pitches()
        .iter()
        .map(|pitch| {
            Ok(PitchFigureTuple {
                figure: FigureTuple::from_pitch_and_reference(pitch, key, bass)?,
                pitch: pitch.clone(),
            })
        })
        .collect()
}

/// The figure of a chord's root corrected for a minor key, where the sixth
/// and seventh degrees are named against the natural minor: music21's
/// `correctRNAlterationForMinor`, in its cautionary reading.
///
/// A raised sixth or seventh loses its sharp, since the lower case numeral
/// a minor or diminished chord takes already says the degree is raised, and
/// a natural one gains a flat as a caution. A chord with a major third keeps
/// its sharp, because `VI` alone would name the chord on the lowered degree.
/// Figures in a major key, and on any other degree, come back as they are.
pub fn correct_rn_alteration_for_minor(
    figure: &FigureTuple,
    key: &Key,
    chord_has_major_third: bool,
) -> FigureTuple {
    if key.mode() != "minor" || !matches!(figure.deg_from_ref_pitch, 6 | 7) {
        return figure.clone();
    }
    if chord_has_major_third && figure.alter >= 1.0 {
        return figure.clone();
    }
    let (alter, prefix) = if figure.alter == 1.0 {
        (0.0, String::new())
    } else if figure.alter == 0.0 {
        (0.0, "b".to_string())
    } else if figure.alter > 1.0 {
        (figure.alter - 1.0, figure.prefix.chars().skip(1).collect())
    } else {
        (figure.alter, format!("b{}", figure.prefix))
    };
    FigureTuple {
        deg_from_ref_pitch: figure.deg_from_ref_pitch,
        alter,
        prefix,
    }
}

/// The inversion figure with the quality mark a chord's fifth and seventh
/// call for in front of it: music21's `correctSuffixForChordQuality`. A
/// diminished fifth writes `o`, an augmented one `+`, and a diminished
/// fifth under a minor seventh `ø`; a figure that already carries the mark
/// is left as it is.
pub fn correct_suffix_for_chord_quality(chord: &Chord, inversion_string: &str) -> String {
    let fifth = chord.semitones_from_chord_step(5);
    let mut quality = match fifth {
        Some(6) => "o",
        Some(8) => "+",
        _ => "",
    };
    let marked = ["o", "°", "/o", "ø"]
        .iter()
        .any(|mark| inversion_string.starts_with(mark));
    if marked && quality == "o" {
        quality = "";
    }
    if fifth == Some(6) && chord.semitones_from_chord_step(7) == Some(10) && quality == "o" {
        quality = "ø";
    }
    format!("{quality}{inversion_string}")
}

/// music21's `identifyAsTonicOrDominant`, so `G B D F` in C major is `V7`
/// and `A C E` in A minor is `i`. A chord containing the tonic is the tonic,
/// one containing the dominant is the dominant, and anything else is judged
/// by how many of its notes the tonic and dominant sevenths share; `None`
/// when neither wins.
pub fn identify_as_tonic_or_dominant(chord: &Chord, key: &Key) -> Result<Option<String>> {
    let names = chord.pitch_names();
    let tonic = key.pitch_from_degree(1)?;
    let dominant = key.pitch_from_degree(5)?;
    let overlap = |figure: &str| -> Result<usize> {
        let members = RomanNumeral::new(figure, key.clone())?
            .to_chord()?
            .pitch_names();
        Ok(members.iter().filter(|name| names.contains(name)).count())
    };
    let is_tonic = if names.contains(&tonic.name()) {
        true
    } else if names.contains(&dominant.name()) {
        false
    } else {
        let (one, five) = (overlap("I7")?, overlap("V7")?);
        if one == five {
            return Ok(None);
        }
        one > five
    };
    let (numeral, root) = if is_tonic {
        (if key.mode() == "minor" { "i" } else { "I" }, &tonic)
    } else {
        ("V", &dominant)
    };
    Ok(Some(format!(
        "{numeral}{}",
        inversion_name_from_root(chord, root, None)
    )))
}

/// Performs functional Roman-numeral analysis in a key.
pub fn analyze_chord(chord: &Chord, key: Key) -> Result<Option<RomanNumeral>> {
    RomanNumeral::analyze(chord, key)
}

/// Performs Roman-numeral analysis in a key using an explicit harmonic root.
pub fn analyze_chord_with_root(
    chord: &Chord,
    key: Key,
    root: &Pitch,
) -> Result<Option<RomanNumeral>> {
    RomanNumeral::analyze_with_root(chord, key, root)
}

pub(super) fn degree_for_root(key: &Key, root: &Pitch) -> Result<Option<(u8, i8)>> {
    let root_pc = pitch_class(root);
    let root_step = root.step();
    let mut best: Option<(u8, i8, bool)> = None;

    for degree in 1..=7 {
        let degree_pitch = key.pitch_from_degree(degree)?;
        let diff = ((root_pc as i16 - pitch_class(&degree_pitch) as i16).rem_euclid(12)) as u8;
        let Some(accidental) = chromatic_diff_to_accidental(diff) else {
            continue;
        };
        let same_step = degree_pitch.step() == root_step;

        let replace = match best {
            None => true,
            Some((_, best_accidental, best_same_step)) => {
                (same_step && !best_same_step)
                    || (same_step == best_same_step && accidental.abs() < best_accidental.abs())
            }
        };
        if replace {
            best = Some((degree as u8, accidental, same_step));
        }
    }

    Ok(best.map(|(degree, accidental, _)| (degree, accidental)))
}

pub(super) fn chromatic_diff_to_accidental(diff: u8) -> Option<i8> {
    match diff {
        0 => Some(0),
        1 => Some(1),
        2 => Some(2),
        10 => Some(-2),
        11 => Some(-1),
        _ => None,
    }
}

pub(super) fn intervals_above_root(chord: &Chord, root_pc: u8) -> Vec<u8> {
    let mut intervals = chord
        .pitch_classes()
        .into_iter()
        .map(|pc| (pc + 12 - root_pc) % 12)
        .collect::<Vec<_>>();
    intervals.sort_unstable();
    intervals.dedup();
    intervals
}

pub(super) fn augmented_sixth_kind_for_key(
    chord: &Chord,
    key: &Key,
) -> Result<Option<AugmentedSixthKind>> {
    let kind = std::iter::once(chord.common_name())
        .chain(chord.common_names())
        .find_map(|name| AugmentedSixthKind::from_common_name(&name));
    let Some(kind) = kind else {
        return Ok(None);
    };

    let pitch_classes = chord.pitch_classes();
    let tonic = key_degree_pitch_class(key, 1, 0)?;
    let lowered_sixth_adjust = if key.mode() == "minor" { 0 } else { -1 };
    let lowered_sixth = key_degree_pitch_class(key, 6, lowered_sixth_adjust)?;
    let raised_fourth = key_degree_pitch_class(key, 4, 1)?;

    if !pitch_classes.contains(&tonic)
        || !pitch_classes.contains(&lowered_sixth)
        || !pitch_classes.contains(&raised_fourth)
    {
        return Ok(None);
    }

    let required_extra = match kind {
        AugmentedSixthKind::Italian => None,
        AugmentedSixthKind::French => Some(key_degree_pitch_class(key, 2, 0)?),
        AugmentedSixthKind::German => {
            let lowered_third_adjust = if key.mode() == "minor" { 0 } else { -1 };
            Some(key_degree_pitch_class(key, 3, lowered_third_adjust)?)
        }
        AugmentedSixthKind::Swiss => Some(key_degree_pitch_class(key, 2, 1)?),
    };

    if required_extra.is_some_and(|pitch_class| !pitch_classes.contains(&pitch_class)) {
        return Ok(None);
    }

    Ok(Some(kind))
}

pub(super) fn key_degree_pitch_class(
    key: &Key,
    degree: usize,
    semitones: IntegerType,
) -> Result<u8> {
    let mut pitch = key.pitch_from_degree(degree)?;
    if semitones != 0 {
        pitch = Interval::from_semitones(semitones)?.transpose_pitch(&pitch)?;
    }
    Ok(pitch_class(&pitch))
}

pub(super) fn roman_inversion(chord: &Chord) -> u8 {
    if chord.pitches().iter().any(|pitch| pitch.octave().is_some()) {
        chord.inversion().unwrap_or(0)
    } else {
        0
    }
}

pub(super) fn symbol_quality(symbol: &ChordSymbol) -> RomanQuality {
    match symbol.quality() {
        ChordQuality::Major
        | ChordQuality::Dominant
        | ChordQuality::Suspended2
        | ChordQuality::Suspended4
        | ChordQuality::Power
        | ChordQuality::Pedal => RomanQuality::Major,
        ChordQuality::Minor => RomanQuality::Minor,
        ChordQuality::Diminished => RomanQuality::Diminished,
        ChordQuality::HalfDiminished => RomanQuality::HalfDiminished,
        ChordQuality::Augmented => RomanQuality::Augmented,
    }
}

pub(super) fn quality_from_intervals(intervals: &[u8]) -> RomanQuality {
    if intervals.contains(&3) && intervals.contains(&6) {
        if intervals.contains(&10) {
            RomanQuality::HalfDiminished
        } else {
            RomanQuality::Diminished
        }
    } else if intervals.contains(&4) && intervals.contains(&8) {
        RomanQuality::Augmented
    } else if intervals.contains(&3) && intervals.contains(&7) {
        RomanQuality::Minor
    } else {
        RomanQuality::Major
    }
}

pub(super) fn roman_figure(
    degree: u8,
    accidental: i8,
    quality: RomanQuality,
    symbol: Option<&ChordSymbol>,
    intervals: &[u8],
    inversion: u8,
) -> String {
    let base = degree_to_roman(degree);
    let prefix = roman_accidental_prefix(accidental);
    let body = roman_body_for_quality(base, quality);
    let suffix = functional_suffix(symbol, intervals, inversion, quality);
    format!("{prefix}{body}{suffix}")
}

pub(super) fn roman_accidental_prefix(accidental: i8) -> String {
    match accidental.cmp(&0) {
        std::cmp::Ordering::Less => "b".repeat(accidental.unsigned_abs() as usize),
        std::cmp::Ordering::Equal => String::new(),
        std::cmp::Ordering::Greater => "#".repeat(accidental as usize),
    }
}

pub(super) fn roman_body_for_quality(base: &str, quality: RomanQuality) -> String {
    match quality {
        RomanQuality::Major => base.to_string(),
        RomanQuality::Minor => base.to_ascii_lowercase(),
        RomanQuality::Diminished => format!("{}o", base.to_ascii_lowercase()),
        RomanQuality::HalfDiminished => format!("{}\u{00f8}", base.to_ascii_lowercase()),
        RomanQuality::Augmented => format!("{base}+"),
    }
}

pub(super) fn functional_suffix(
    symbol: Option<&ChordSymbol>,
    intervals: &[u8],
    inversion: u8,
    quality: RomanQuality,
) -> String {
    if let Some(symbol) = symbol
        && needs_chord_symbol_suffix(symbol)
    {
        return chord_symbol_suffix_for_roman(symbol, quality);
    }
    figured_bass_suffix(intervals, inversion, quality)
}

pub(super) fn needs_chord_symbol_suffix(symbol: &ChordSymbol) -> bool {
    matches!(
        symbol.quality(),
        ChordQuality::Suspended2 | ChordQuality::Suspended4 | ChordQuality::Power
    ) || !symbol.additions().is_empty()
        || symbol.alterations().iter().any(|alteration| {
            !(matches!(symbol.quality(), ChordQuality::HalfDiminished)
                && alteration.degree() == 5
                && alteration.semitones() == -1)
        })
        || symbol.extensions().iter().any(|degree| *degree != 7)
        || chord_symbol_suffix(symbol).contains("maj7")
}

pub(super) fn chord_symbol_suffix_for_roman(symbol: &ChordSymbol, quality: RomanQuality) -> String {
    let suffix = chord_symbol_suffix(symbol);
    let converted = match quality {
        RomanQuality::Major => suffix.to_string(),
        RomanQuality::Minor => suffix
            .strip_prefix('m')
            .filter(|rest| !rest.starts_with("aj"))
            .unwrap_or(suffix)
            .to_string(),
        RomanQuality::Diminished => suffix.strip_prefix("dim").unwrap_or(suffix).to_string(),
        RomanQuality::HalfDiminished => {
            suffix.strip_prefix('m').unwrap_or(suffix).replace("b5", "")
        }
        RomanQuality::Augmented => suffix
            .strip_prefix("aug")
            .or_else(|| suffix.strip_prefix('+'))
            .unwrap_or(suffix)
            .to_string(),
    };

    if converted == "6" {
        " add(13)".to_string()
    } else {
        converted
    }
}

pub(super) fn chord_symbol_suffix(symbol: &ChordSymbol) -> &str {
    let body = symbol
        .figure()
        .split_once('/')
        .map_or(symbol.figure(), |(body, _)| body);
    let root_name = normalize_symbol_root_name(&symbol.root().name());
    body.strip_prefix(&root_name).unwrap_or(body)
}

pub(super) fn normalize_symbol_root_name(name: &str) -> String {
    name.replace('-', "b")
}

pub(super) fn figured_bass_suffix(
    intervals: &[u8],
    inversion: u8,
    quality: RomanQuality,
) -> String {
    if has_seventh(intervals) {
        let suffix = match inversion {
            1 => "65",
            2 => "43",
            3 => "42",
            _ => "7",
        };
        if matches!(quality, RomanQuality::Major) && intervals.contains(&11) {
            format!("maj{suffix}")
        } else {
            suffix.to_string()
        }
    } else if has_triad_shape(intervals) {
        match inversion {
            1 => "6".to_string(),
            2 => "64".to_string(),
            _ => String::new(),
        }
    } else {
        String::new()
    }
}

pub(super) fn has_seventh(intervals: &[u8]) -> bool {
    intervals.contains(&10) || intervals.contains(&11) || intervals.contains(&9)
}

pub(super) fn has_triad_shape(intervals: &[u8]) -> bool {
    (intervals.contains(&3) || intervals.contains(&4))
        && intervals.iter().any(|interval| matches!(interval, 6..=8))
}

pub(super) fn normalize_pitch_name(name: &str) -> String {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    let mut normalized = first.to_string();
    for ch in chars {
        if ch == 'b' {
            normalized.push('-');
        } else {
            normalized.push(ch);
        }
    }
    normalized
}
