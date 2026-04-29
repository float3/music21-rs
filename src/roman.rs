use crate::{
    chord::Chord,
    chordsymbol::{ChordQuality, ChordSymbol},
    defaults::IntegerType,
    error::{Error, Result},
    interval::Interval,
    key::Key,
    pitch::Pitch,
};
use std::fmt;

/// A parsed Roman numeral in a key.
#[derive(Clone, Debug)]
pub struct RomanNumeral {
    figure: String,
    key: Key,
    degree: u8,
    accidental: i8,
    inversion: u8,
    seventh: bool,
    quality: RomanQuality,
    secondary: Option<String>,
    kind: RomanKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RomanKind {
    Diatonic,
    AugmentedSixth(AugmentedSixthKind),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AugmentedSixthKind {
    Italian,
    French,
    German,
    Swiss,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RomanQuality {
    Major,
    Minor,
    Diminished,
    HalfDiminished,
    Augmented,
}

impl AugmentedSixthKind {
    fn from_figure(figure: &str) -> Option<Self> {
        match figure.trim() {
            "It+6" | "It6" => Some(Self::Italian),
            "Fr+6" | "Fr6" => Some(Self::French),
            "Ger+6" | "Ger6" => Some(Self::German),
            "Sw+6" | "Sw6" => Some(Self::Swiss),
            _ => None,
        }
    }

    fn from_common_name(name: &str) -> Option<Self> {
        if name.contains("Italian augmented sixth chord") {
            Some(Self::Italian)
        } else if name.contains("French augmented sixth chord") {
            Some(Self::French)
        } else if name.contains("German augmented sixth chord") {
            Some(Self::German)
        } else if name.contains("Swiss augmented sixth chord") {
            Some(Self::Swiss)
        } else {
            None
        }
    }

    fn figure(self) -> &'static str {
        match self {
            Self::Italian => "It+6",
            Self::French => "Fr+6",
            Self::German => "Ger+6",
            Self::Swiss => "Sw+6",
        }
    }

    fn interval_names(self) -> Vec<&'static str> {
        match self {
            Self::Italian => vec!["P1", "M3", "a6"],
            Self::French => vec!["P1", "M3", "a4", "a6"],
            Self::German => vec!["P1", "M3", "P5", "a6"],
            Self::Swiss => vec!["P1", "M3", "aa4", "a6"],
        }
    }
}

impl RomanNumeral {
    /// Parses a Roman numeral figure in a key.
    ///
    /// Supports ordinary figures such as `V7/V` and augmented-sixth figures
    /// such as `It+6`, `Fr+6`, `Ger+6`, and `Sw+6`.
    pub fn new(figure: impl Into<String>, key: Key) -> Result<Self> {
        let figure = figure.into();
        let trimmed = figure.trim();
        if trimmed.is_empty() {
            return Err(Error::Chord("roman numeral cannot be empty".to_string()));
        }

        if let Some(kind) = AugmentedSixthKind::from_figure(trimmed) {
            return Ok(Self {
                figure: kind.figure().to_string(),
                key,
                degree: 6,
                accidental: -1,
                inversion: 0,
                seventh: false,
                quality: RomanQuality::Augmented,
                secondary: None,
                kind: RomanKind::AugmentedSixth(kind),
            });
        }

        let (primary, secondary) = match trimmed.split_once('/') {
            Some((primary, secondary)) => (primary, Some(secondary.to_string())),
            None => (trimmed, None),
        };

        let (accidental, primary) = split_roman_accidental_prefix(primary);
        let (roman, suffix) = split_roman_prefix(primary)?;
        let degree = roman_degree(roman)?;
        let quality = roman_quality(roman, suffix);
        let inversion = parse_inversion(suffix);
        let seventh = suffix_has_seventh(suffix);

        Ok(Self {
            figure: trimmed.to_string(),
            key,
            degree,
            accidental,
            inversion,
            seventh,
            quality,
            secondary,
            kind: RomanKind::Diatonic,
        })
    }

    /// Returns the original figure.
    pub fn figure(&self) -> &str {
        &self.figure
    }

    /// Returns the one-based scale degree.
    pub fn degree(&self) -> u8 {
        self.degree
    }

    /// Returns the chromatic alteration of the scale degree in semitones.
    ///
    /// Negative values are flats and positive values are sharps, so `bII`
    /// returns `-1` and `#iv` returns `1`.
    pub fn accidental(&self) -> i8 {
        self.accidental
    }

    /// Returns the inversion number, where root position is `0`.
    pub fn inversion(&self) -> u8 {
        self.inversion
    }

    /// Returns the secondary/applied target figure, if any.
    pub fn secondary(&self) -> Option<&str> {
        self.secondary.as_deref()
    }

    /// Returns the key context.
    pub fn key(&self) -> &Key {
        &self.key
    }

    /// Realizes the Roman numeral as a chord.
    pub fn to_chord(&self) -> Result<Chord> {
        if let RomanKind::AugmentedSixth(kind) = self.kind {
            return self.augmented_sixth_chord(kind);
        }

        let effective_key = self.effective_key()?;
        let mut root = effective_key.pitch_from_degree(self.degree as usize)?;
        if self.accidental != 0 {
            root =
                Interval::from_semitones(self.accidental as IntegerType)?.transpose_pitch(&root)?;
        }
        let mut pitches = self
            .interval_names()
            .into_iter()
            .map(|name| Interval::from_name(name)?.transpose_pitch(&root))
            .collect::<Result<Vec<_>>>()?;

        for _ in 0..self.inversion.min(pitches.len().saturating_sub(1) as u8) {
            let pitch = pitches.remove(0);
            let transposed = Interval::from_name("P8")?.transpose_pitch(&pitch)?;
            pitches.push(transposed);
        }

        Chord::new(pitches.as_slice())
    }

    fn augmented_sixth_chord(&self, kind: AugmentedSixthKind) -> Result<Chord> {
        let mut lowered_sixth = self.key.pitch_from_degree(6)?;
        if self.key.mode() != "minor" {
            lowered_sixth = Interval::from_semitones(-1)?.transpose_pitch(&lowered_sixth)?;
        }
        let pitches = kind
            .interval_names()
            .into_iter()
            .map(|name| Interval::from_name(name)?.transpose_pitch(&lowered_sixth))
            .collect::<Result<Vec<_>>>()?;
        Chord::new(pitches.as_slice())
    }

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

    fn effective_key(&self) -> Result<Key> {
        let Some(secondary) = &self.secondary else {
            return Ok(self.key.clone());
        };

        let (accidental, secondary) = split_roman_accidental_prefix(secondary);
        let (roman, _) = split_roman_prefix(secondary)?;
        let degree = roman_degree(roman)?;
        let mut tonic = self.key.pitch_from_degree(degree as usize)?;
        if accidental != 0 {
            tonic = Interval::from_semitones(accidental as IntegerType)?.transpose_pitch(&tonic)?;
        }
        let mode = if roman.chars().next().is_some_and(char::is_uppercase) {
            "major"
        } else {
            "minor"
        };
        Key::from_tonic_mode(&tonic.name(), mode)
    }

    fn interval_names(&self) -> Vec<&'static str> {
        match (self.quality, self.seventh) {
            (RomanQuality::Major, false) => vec!["P1", "M3", "P5"],
            (RomanQuality::Major, true) => vec!["P1", "M3", "P5", "m7"],
            (RomanQuality::Minor, false) => vec!["P1", "m3", "P5"],
            (RomanQuality::Minor, true) => vec!["P1", "m3", "P5", "m7"],
            (RomanQuality::Diminished, false) => vec!["P1", "m3", "d5"],
            (RomanQuality::Diminished, true) => vec!["P1", "m3", "d5", "d7"],
            (RomanQuality::HalfDiminished, false) => vec!["P1", "m3", "d5"],
            (RomanQuality::HalfDiminished, true) => vec!["P1", "m3", "d5", "m7"],
            (RomanQuality::Augmented, false) => vec!["P1", "M3", "a5"],
            (RomanQuality::Augmented, true) => vec!["P1", "M3", "a5", "m7"],
        }
    }
}

impl fmt::Display for RomanNumeral {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.figure())
    }
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

fn split_roman_accidental_prefix(value: &str) -> (i8, &str) {
    let mut accidental = 0;
    let mut end = 0;
    for (idx, ch) in value.char_indices() {
        match ch {
            '#' => {
                accidental += 1;
                end = idx + ch.len_utf8();
            }
            'b' | '-' => {
                accidental -= 1;
                end = idx + ch.len_utf8();
            }
            _ => break,
        }
    }
    (accidental, &value[end..])
}

fn split_roman_prefix(value: &str) -> Result<(&str, &str)> {
    let end = value
        .char_indices()
        .find_map(|(idx, ch)| (!matches!(ch, 'I' | 'V' | 'X' | 'i' | 'v' | 'x')).then_some(idx))
        .unwrap_or(value.len());

    if end == 0 {
        return Err(Error::Chord(format!("missing roman numeral in {value:?}")));
    }

    Ok((&value[..end], &value[end..]))
}

fn roman_degree(roman: &str) -> Result<u8> {
    match roman.to_ascii_uppercase().as_str() {
        "I" => Ok(1),
        "II" => Ok(2),
        "III" => Ok(3),
        "IV" => Ok(4),
        "V" => Ok(5),
        "VI" => Ok(6),
        "VII" => Ok(7),
        _ => Err(Error::Chord(format!("unsupported roman numeral {roman:?}"))),
    }
}

fn roman_quality(roman: &str, suffix: &str) -> RomanQuality {
    let lower = suffix.to_ascii_lowercase();
    if suffix.contains('\u{00f8}') || lower.contains("m7b5") {
        RomanQuality::HalfDiminished
    } else if lower.contains('o') || lower.contains("dim") {
        RomanQuality::Diminished
    } else if lower.contains('+') || lower.contains("aug") {
        RomanQuality::Augmented
    } else if roman.chars().next().is_some_and(char::is_lowercase) {
        RomanQuality::Minor
    } else {
        RomanQuality::Major
    }
}

fn suffix_has_seventh(suffix: &str) -> bool {
    let suffix = strip_roman_addition_groups(suffix);
    suffix.contains('7')
        || suffix.contains('9')
        || suffix.contains("11")
        || suffix.contains("13")
        || suffix.contains("65")
        || suffix.contains("43")
        || suffix.contains("42")
}

fn parse_inversion(suffix: &str) -> u8 {
    let suffix = strip_roman_addition_groups(suffix);
    if suffix.contains("64") || suffix.contains("43") {
        2
    } else if suffix.contains("65") || suffix.contains('6') {
        1
    } else if suffix.contains("42") {
        3
    } else {
        0
    }
}

fn strip_roman_addition_groups(suffix: &str) -> String {
    let mut stripped = String::with_capacity(suffix.len());
    let mut rest = suffix;
    while let Some(index) = rest.find("add(") {
        stripped.push_str(&rest[..index]);
        let addition = &rest[index + 4..];
        let Some(end) = addition.find(')') else {
            rest = addition;
            continue;
        };
        rest = &addition[end + 1..];
    }
    stripped.push_str(rest);
    stripped
}

fn degree_for_root(key: &Key, root: &Pitch) -> Result<Option<(u8, i8)>> {
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

fn chromatic_diff_to_accidental(diff: u8) -> Option<i8> {
    match diff {
        0 => Some(0),
        1 => Some(1),
        2 => Some(2),
        10 => Some(-2),
        11 => Some(-1),
        _ => None,
    }
}

fn intervals_above_root(chord: &Chord, root_pc: u8) -> Vec<u8> {
    let mut intervals = chord
        .pitch_classes()
        .into_iter()
        .map(|pc| (pc + 12 - root_pc) % 12)
        .collect::<Vec<_>>();
    intervals.sort_unstable();
    intervals.dedup();
    intervals
}

fn augmented_sixth_kind_for_key(chord: &Chord, key: &Key) -> Result<Option<AugmentedSixthKind>> {
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

fn key_degree_pitch_class(key: &Key, degree: usize, semitones: IntegerType) -> Result<u8> {
    let mut pitch = key.pitch_from_degree(degree)?;
    if semitones != 0 {
        pitch = Interval::from_semitones(semitones)?.transpose_pitch(&pitch)?;
    }
    Ok(pitch_class(&pitch))
}

fn roman_inversion(chord: &Chord) -> u8 {
    if chord.pitches().iter().any(|pitch| pitch.octave().is_some()) {
        chord.inversion().unwrap_or(0)
    } else {
        0
    }
}

fn symbol_quality(symbol: &ChordSymbol) -> RomanQuality {
    match symbol.quality() {
        ChordQuality::Major
        | ChordQuality::Dominant
        | ChordQuality::Suspended2
        | ChordQuality::Suspended4
        | ChordQuality::Power => RomanQuality::Major,
        ChordQuality::Minor => RomanQuality::Minor,
        ChordQuality::Diminished => RomanQuality::Diminished,
        ChordQuality::HalfDiminished => RomanQuality::HalfDiminished,
        ChordQuality::Augmented => RomanQuality::Augmented,
    }
}

fn quality_from_intervals(intervals: &[u8]) -> RomanQuality {
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

fn roman_figure(
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

fn roman_accidental_prefix(accidental: i8) -> String {
    match accidental.cmp(&0) {
        std::cmp::Ordering::Less => "b".repeat(accidental.unsigned_abs() as usize),
        std::cmp::Ordering::Equal => String::new(),
        std::cmp::Ordering::Greater => "#".repeat(accidental as usize),
    }
}

fn roman_body_for_quality(base: &str, quality: RomanQuality) -> String {
    match quality {
        RomanQuality::Major => base.to_string(),
        RomanQuality::Minor => base.to_ascii_lowercase(),
        RomanQuality::Diminished => format!("{}o", base.to_ascii_lowercase()),
        RomanQuality::HalfDiminished => format!("{}\u{00f8}", base.to_ascii_lowercase()),
        RomanQuality::Augmented => format!("{base}+"),
    }
}

fn functional_suffix(
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

fn needs_chord_symbol_suffix(symbol: &ChordSymbol) -> bool {
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

fn chord_symbol_suffix_for_roman(symbol: &ChordSymbol, quality: RomanQuality) -> String {
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

fn chord_symbol_suffix(symbol: &ChordSymbol) -> &str {
    let body = symbol
        .figure()
        .split_once('/')
        .map_or(symbol.figure(), |(body, _)| body);
    let root_name = normalize_symbol_root_name(&symbol.root().name());
    body.strip_prefix(&root_name).unwrap_or(body)
}

fn normalize_symbol_root_name(name: &str) -> String {
    name.replace('-', "b")
}

fn figured_bass_suffix(intervals: &[u8], inversion: u8, quality: RomanQuality) -> String {
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

fn has_seventh(intervals: &[u8]) -> bool {
    intervals.contains(&10) || intervals.contains(&11) || intervals.contains(&9)
}

fn has_triad_shape(intervals: &[u8]) -> bool {
    (intervals.contains(&3) || intervals.contains(&4))
        && intervals.iter().any(|interval| matches!(interval, 6..=8))
}

fn degree_to_roman(degree: u8) -> &'static str {
    match degree {
        1 => "I",
        2 => "II",
        3 => "III",
        4 => "IV",
        5 => "V",
        6 => "VI",
        7 => "VII",
        _ => "I",
    }
}

fn normalize_pitch_name(name: &str) -> String {
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

fn pitch_class(pitch: &Pitch) -> u8 {
    (pitch.ps().round() as IntegerType).rem_euclid(12) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secondary_dominant_resolves_to_chord() {
        let key = Key::from_tonic_mode("C", "major").unwrap();
        let rn = RomanNumeral::new("V7/V", key).unwrap();
        assert_eq!(rn.degree(), 5);
        assert_eq!(rn.secondary(), Some("V"));
        assert_eq!(
            rn.to_chord().unwrap().pitched_common_name(),
            "D-dominant seventh chord"
        );
    }

    #[test]
    fn analyzes_chord_in_key() {
        let key = Key::from_tonic_mode("C", "major").unwrap();
        let chord = Chord::new("G B D F").unwrap();
        let rn = RomanNumeral::analyze(&chord, key).unwrap().unwrap();
        assert_eq!(rn.figure(), "V7");
    }

    #[test]
    fn analyzes_accidentals_inversions_and_half_diminished_quality() {
        let key = Key::from_tonic_mode("C", "major").unwrap();

        let neapolitan = Chord::new("D- F A-").unwrap();
        let rn = RomanNumeral::analyze(&neapolitan, key.clone())
            .unwrap()
            .unwrap();
        assert_eq!(rn.figure(), "bII");
        assert_eq!(rn.degree(), 2);
        assert_eq!(rn.accidental(), -1);

        let first_inversion = Chord::new("E4 G4 C5").unwrap();
        let rn = RomanNumeral::analyze(&first_inversion, key.clone())
            .unwrap()
            .unwrap();
        assert_eq!(rn.figure(), "I6");

        let leading_tone = Chord::new("B D F A").unwrap();
        let rn = RomanNumeral::analyze(&leading_tone, key).unwrap().unwrap();
        assert_eq!(rn.figure(), "vii\u{00f8}7");
    }

    #[test]
    fn analyzes_with_explicit_root_for_browser_style_sets() {
        let key = Key::from_tonic_mode("C", "major").unwrap();
        let root = Pitch::from_name("C").unwrap();
        let chord = Chord::new("C D").unwrap();
        let rn = RomanNumeral::analyze_with_root(&chord, key.clone(), &root)
            .unwrap()
            .unwrap();
        assert_eq!(rn.figure(), "I add(9)");

        let sixth = Chord::new("C A").unwrap();
        let rn = RomanNumeral::analyze_with_root(&sixth, key, &root)
            .unwrap()
            .unwrap();
        assert_eq!(rn.figure(), "I add(13)");
    }

    #[test]
    fn analyzes_augmented_sixth_chords_functionally() {
        let key = Key::from_tonic_mode("C", "minor").unwrap();
        let root = Pitch::from_name("C").unwrap();
        let french = Chord::new("C D F# A-").unwrap();
        let rn = RomanNumeral::analyze_with_root(&french, key.clone(), &root)
            .unwrap()
            .unwrap();
        assert_eq!(rn.figure(), "Fr+6");

        let german = Chord::new("A- C E- F#").unwrap();
        let rn = RomanNumeral::analyze(&german, key).unwrap().unwrap();
        assert_eq!(rn.figure(), "Ger+6");
    }

    #[test]
    fn roman_numerals_parse_inversions_and_qualities() {
        let key = Key::from_tonic_mode("C", "major").unwrap();
        let first_inversion = RomanNumeral::new("I6", key.clone()).unwrap();
        assert_eq!(first_inversion.inversion(), 1);
        assert_eq!(
            first_inversion
                .to_chord()
                .unwrap()
                .pitches()
                .into_iter()
                .map(|pitch| pitch.name())
                .collect::<Vec<_>>(),
            vec!["E", "G", "C"]
        );

        let diminished = RomanNumeral::new("viio7", key.clone()).unwrap();
        assert_eq!(diminished.degree(), 7);
        assert!(
            diminished
                .to_chord()
                .unwrap()
                .common_name()
                .contains("diminished")
        );

        let half_diminished = RomanNumeral::new("vii\u{00f8}7", key.clone()).unwrap();
        assert_eq!(half_diminished.degree(), 7);
        assert_eq!(half_diminished.accidental(), 0);
        assert!(
            half_diminished
                .to_chord()
                .unwrap()
                .common_name()
                .contains("half-diminished")
        );

        let borrowed = RomanNumeral::new("bII", key.clone()).unwrap();
        assert_eq!(borrowed.degree(), 2);
        assert_eq!(borrowed.accidental(), -1);

        let added_thirteenth = RomanNumeral::new("I add(13)", key.clone()).unwrap();
        assert_eq!(added_thirteenth.inversion(), 0);
        assert_eq!(
            added_thirteenth.to_chord().unwrap().common_name(),
            "major triad"
        );

        let augmented = RomanNumeral::new("III+", key).unwrap();
        assert_eq!(
            augmented
                .to_chord()
                .unwrap()
                .pitches()
                .into_iter()
                .map(|pitch| pitch.name())
                .collect::<Vec<_>>(),
            vec!["E", "G#", "B#"]
        );
    }

    #[test]
    fn roman_numerals_parse_augmented_sixth_figures() {
        let key = Key::from_tonic_mode("C", "minor").unwrap();
        let french = RomanNumeral::new("Fr+6", key).unwrap();
        assert_eq!(french.degree(), 6);
        assert_eq!(french.accidental(), -1);
        assert_eq!(
            french
                .to_chord()
                .unwrap()
                .pitches()
                .into_iter()
                .map(|pitch| pitch.name())
                .collect::<Vec<_>>(),
            vec!["A-", "C", "D", "F#"]
        );
    }

    #[test]
    fn roman_numerals_report_invalid_figures_and_empty_analysis() {
        let key = Key::from_tonic_mode("C", "major").unwrap();

        assert!(RomanNumeral::new("", key.clone()).is_err());
        assert!(RomanNumeral::new("Q", key.clone()).is_err());
        assert!(
            analyze_chord(&Chord::empty().unwrap(), key)
                .unwrap()
                .is_none()
        );
    }
}
