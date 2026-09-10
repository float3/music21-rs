//! Reading a figure: the accidental in front of the numeral, the
//! numeral itself, the inversion and figured-bass digits after it, the
//! bracketed omissions, additions and alterations, and the applied part.

use super::*;

impl RomanNumeral {
    /// Raises the sixth and seventh degrees of a minor key where the figure
    /// asks for a chord that only the raised degree gives.
    ///
    /// This is music21's `_adjustMinorVIandVIIByQuality` at its default
    /// setting, and it is what makes `viio7` in A minor the diminished
    /// seventh on `G#` rather than a chord on `G` with three flats in it,
    /// and `vi` the minor triad on `F#`. The natural degrees are what `VI`
    /// and `VII` ask for, and those are left alone.
    pub(super) fn raise_minor_sixth_and_seventh(&mut self, column: &mut String) -> Result<()> {
        if !matches!(self.degree, 6 | 7) {
            return Ok(());
        }
        // A numeral read against a collection takes that collection's
        // degrees as they are written. Raising the sixth and the seventh is
        // a rule about minor *keys* — music21 asks the thing it was given
        // for its mode, and a scale has none.
        if self.scale.is_some() {
            return Ok(());
        }
        // Against the key the figure is actually read in, so the `vi` of a
        // secondary numeral is judged in the key that numeral establishes.
        if !self.case_matters {
            return Ok(());
        }
        let reading = if self.degree == 6 {
            self.sixth_minor
        } else {
            self.seventh_minor
        };
        let adjusted = adjust_minor_vi_and_vii_by_quality(
            &self.effective_key()?,
            reading,
            self.implied_quality,
            self.accidental,
        );
        if adjusted != self.accidental {
            self.accidental = adjusted;
            sharpen_figure(column);
        }
        Ok(())
    }
}

/// The accidental a numeral on the sixth or seventh degree of a minor key
/// ends up with, given how the key is read and the chord the figure asks
/// for: music21's `adjustMinorVIandVIIByQuality`.
///
/// A minor triad on the sixth or a diminished chord on the seventh is one
/// only the raised degree gives, so under the `Quality` reading such a
/// figure takes a sharp and a major one takes the natural degree. `Flat` and
/// `Sharp` decide without looking at the chord, and `Cautionary` reads an
/// accidental already written as a caution rather than as a further change:
/// a sharp says nothing more, and a flat cancels the raise, so the two meet
/// at the natural degree. In a major key the accidental stands as written.
pub fn adjust_minor_vi_and_vii_by_quality(
    key: &Key,
    reading: Minor67Default,
    quality: ImpliedQuality,
    accidental: i8,
) -> i8 {
    if key.mode() != "minor" {
        return accidental;
    }
    let wants_raised = matches!(
        quality,
        ImpliedQuality::Minor | ImpliedQuality::Diminished | ImpliedQuality::HalfDiminished
    );
    let raise = match reading {
        Minor67Default::Flat => false,
        Minor67Default::Sharp => true,
        Minor67Default::Quality => wants_raised,
        Minor67Default::Cautionary => match accidental {
            0 => wants_raised,
            sharps if sharps >= 1 => false,
            _ => true,
        },
    };
    if raise { accidental + 1 } else { accidental }
}

/// Splits the accidentals written in front of a roman numeral off the rest of
/// the figure, counting sharps as positive and flats as negative.
pub fn split_roman_accidental_prefix(value: &str) -> (i8, &str) {
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

/// The chords music21 writes by name, as the figures they stand for.
///
/// The Neapolitan is the flattened second degree, written `N` and — the way
/// it is nearly always used — `N6` in first inversion. The cadential
/// six-four is the tonic triad in second inversion, and takes the case of
/// the key it stands in.
pub(super) fn named_figure(figure: &str, key: &Key) -> String {
    let tonic = if key.mode() == "minor" { "i" } else { "I" };
    match figure {
        "N" | "N6" => "bII6".to_string(),
        "N53" => "bII".to_string(),
        "Cad64" => format!("{tonic}64"),
        other => other.to_string(),
    }
}

/// music21's "immediate fixes" to a figure as written.
///
/// A diminished chord is written `o`, `0`, `º` or `°`, and a half-diminished
/// one `ø` or `/o`; each pair is folded onto one spelling, and the fold is
/// what the numeral reports back as its figure. The zero is left alone when a
/// digit precedes it, so `10` stays a ten.
pub(super) fn fold_figure_symbols(figure: &str) -> String {
    let mut folded = String::with_capacity(figure.len());
    let mut previous: Option<char> = None;
    for letter in figure.chars() {
        let fixed = match letter {
            '0' if !previous.is_some_and(|before| before.is_ascii_digit()) => 'o',
            '\u{00ba}' | '\u{00b0}' => 'o',
            other => other,
        };
        folded.push(fixed);
        previous = Some(letter);
    }
    folded.replace("/o", "\u{00f8}")
}

/// music21's figure validation: letters, digits and the handful of symbols a
/// figure is written with, and never the letters no numeral contains.
///
/// The parentheses and spaces of this crate's own `add(...)` and `omit(...)`
/// groups come through as well, since a figure written that way is one it
/// can read.
pub(super) fn validate_figure(figure: &str) -> Result<()> {
    let ok = figure.chars().all(|letter| {
        letter.is_alphanumeric()
            || matches!(
                letter,
                '#' | '\u{00b0}' | '+' | '-' | '/' | '[' | ']' | '(' | ')' | ' '
            )
    });
    if !ok
        || figure
            .chars()
            .any(|letter| matches!(letter, 'x' | 'y' | 'z'))
    {
        return Err(Error::Chord(format!("Invalid figure: {figure}")));
    }
    Ok(())
}

/// The chords a figured-bass column implies a root for.
///
/// music21's `FIGURES_IMPLYING_ROOT`. Every other column — `54`, say — is
/// read as a stack over its bass, and the bass is then the root.
pub(super) const FIGURES_IMPLYING_ROOT: &[&[u8]] = &[
    // triads
    &[6],
    &[6, 3],
    &[6, 4],
    // seventh chords
    &[6, 5, 3],
    &[6, 5],
    &[6, 4, 3],
    &[4, 3],
    &[6, 4, 2],
    &[4, 2],
    &[2],
    // ninth chords
    &[7, 6, 5, 3],
    &[6, 5, 4, 3],
    &[6, 4, 3, 2],
    &[7, 5, 3, 2],
    // eleventh chords
    &[9, 7, 6, 5, 3],
    &[7, 6, 5, 4, 3],
    &[9, 6, 5, 4, 3],
    &[9, 7, 6, 4, 3],
    &[7, 6, 5, 4, 2],
];

/// Where music21 splits a secondary numeral off a figure.
///
/// Its own regular expression asks for a letter after the slash, and
/// deliberately not `o` — so `vii/o7` is a half-diminished seventh and not a
/// numeral applied to some chord `o7` — and not a digit, so the `6/5` of
/// `Ger6/5` stays one figure.
pub fn split_secondary(figure: &str) -> (&str, Option<String>) {
    for (index, letter) in figure.char_indices() {
        if letter != '/' {
            continue;
        }
        let rest = &figure[index + 1..];
        let opens = rest.chars().next().is_some_and(|next| {
            next == '#' || (next.is_ascii_alphabetic() && next != 'o' && next != 'O')
        });
        if opens {
            return (&figure[..index], Some(rest.to_string()));
        }
    }
    (figure, None)
}

/// Takes the `[noN]` groups out of a figure, leaving the rest of it.
///
/// The steps are counted from the root and folded into one octave, so
/// `[no11]` leaves out the fourth.
pub fn take_omitted_steps(figure: &mut String) -> Vec<u8> {
    let mut steps = Vec::new();
    let mut kept = String::with_capacity(figure.len());
    let mut remaining = figure.as_str();
    while let Some(start) = remaining.find("[no") {
        let Some(end) = remaining[start..].find(']') else {
            break;
        };
        let group = &remaining[start + 1..start + end];
        for part in group.split("no") {
            if let Ok(step) = part.trim().parse::<u8>() {
                steps.push(if step % 7 == 0 { 7 } else { step % 7 });
            }
        }
        kept.push_str(&remaining[..start]);
        remaining = remaining[start + end + 1..].trim_start();
    }
    kept.push_str(remaining);
    *figure = kept;
    steps
}

/// Takes the `[addN]` groups out of a figure, with the accidental each was
/// written with.
pub fn take_added_steps(figure: &mut String) -> Vec<(i8, u8)> {
    take_bracket_groups(figure, "[add")
}

/// Takes the `[#N]` and `[bN]` groups out of a figure.
pub fn take_bracketed_alterations(figure: &mut String) -> Vec<(i8, u8)> {
    take_bracket_groups(figure, "[")
}

/// The shape the two share: a bracket, a prefix, some accidentals, a number,
/// and a closing bracket. A bracket holding anything else is left where it is.
pub(super) fn take_bracket_groups(figure: &mut String, opening: &str) -> Vec<(i8, u8)> {
    let mut groups = Vec::new();
    let mut kept = String::with_capacity(figure.len());
    let mut remaining = figure.as_str();
    while let Some(start) = remaining.find(opening) {
        let Some(end) = remaining[start..].find(']') else {
            break;
        };
        let body = &remaining[start + opening.len()..start + end];
        let digits: String = body.chars().filter(char::is_ascii_digit).collect();
        let well_formed = !digits.is_empty()
            && body
                .chars()
                .all(|letter| matches!(letter, '#' | 'b' | '-') || letter.is_ascii_digit());
        if !well_formed {
            kept.push_str(&remaining[..start + end + 1]);
            remaining = &remaining[start + end + 1..];
            continue;
        }
        let alter: i8 = body
            .chars()
            .map(|letter| match letter {
                '#' => 1,
                'b' | '-' => -1,
                _ => 0,
            })
            .sum();
        if let Ok(step) = digits.parse::<u8>() {
            groups.push((alter, step));
        }
        kept.push_str(&remaining[..start]);
        remaining = remaining[start + end + 1..].trim_start();
    }
    kept.push_str(remaining);
    *figure = kept;
    groups
}

/// music21's `expandShortHand`: the figures of a column, one string each.
pub fn expand_shorthand(shorthand: &str) -> Vec<String> {
    let mut shorthand = shorthand.replace('/', "");
    // A lone flat is a flattened third.
    if shorthand == "b" || shorthand == "-" {
        shorthand.push('3');
    }
    let letters: Vec<char> = shorthand.chars().collect();
    let mut tokens = Vec::new();
    let mut index = 0;
    while index < letters.len() {
        let start = index;
        for mark in ['#', '-', 'b', 'o'] {
            while letters.get(index) == Some(&mark) {
                index += 1;
            }
        }
        // The two-digit figures are read whole, so `13` is a thirteenth and
        // not a one and a three.
        let read = match letters.get(index) {
            Some('1') if matches!(letters.get(index + 1), Some('1' | '3' | '5')) => {
                index += 2;
                true
            }
            Some(digit) if digit.is_ascii_digit() && *digit != '0' => {
                index += 1;
                true
            }
            _ => false,
        };
        if read {
            tokens.push(letters[start..index].iter().collect::<String>());
        } else {
            index = start + 1;
        }
    }
    // A column written as a third alone is a fifth and a third — the third
    // itself, not any figure whose digits happen to end in one: `13` is a
    // thirteenth and stands for the whole stack under it.
    if tokens.len() == 1 && tokens[0].trim_start_matches(['#', '-', 'b', 'o']) == "3" {
        tokens.insert(0, "5".to_string());
    }
    tokens
}

/// The key a secondary numeral establishes.
///
/// music21 reads the numeral after the slash as a chord of its own and takes
/// the key from that chord's root and quality, so the `vi` of `V/vi` in C
/// minor is the raised sixth degree the same way a plain `vi` would be.
pub fn secondary_key(
    key: &Key,
    secondary: &str,
    sixth_minor: Minor67Default,
    seventh_minor: Minor67Default,
    case_matters: bool,
) -> Result<Key> {
    let numeral = RomanNumeral::with_options(
        secondary.to_string(),
        key.clone(),
        sixth_minor,
        seventh_minor,
        case_matters,
    )?;
    let chord = numeral.to_chord()?;
    let root = chord
        .root()
        .ok_or_else(|| Error::Chord(format!("no root for secondary numeral {secondary}")))?;
    let mode = match numeral.implied_quality {
        ImpliedQuality::Minor => "minor",
        ImpliedQuality::Major => "major",
        _ if chord.semitones_from_chord_step(3) == Some(3) => "minor",
        _ => "major",
    };
    Key::from_tonic_mode(&root.name(), mode)
}

/// Reads the numeral off the front of a figure.
///
/// music21's `_parseRNAloneAmidstAug6`, which is where the augmented sixths
/// stop being numerals: `Ger` alone means `Ger65`, since that is the position
/// the chord is nearly always written in, and `Fr6` means `Fr43` for the same
/// reason.
pub fn parse_numeral_alone(figure: &str) -> Result<NumeralAlone> {
    if let Some(kind) = augmented_sixth_prefix(figure) {
        let (name, default) = match kind {
            AugmentedSixthKind::Italian => ("It", "6"),
            AugmentedSixthKind::French => ("Fr", "43"),
            AugmentedSixthKind::German => ("Ger", "65"),
            AugmentedSixthKind::Swiss => ("Sw", "43"),
        };
        let (degree, alteration) = match kind {
            AugmentedSixthKind::Italian | AugmentedSixthKind::German => (4, 1),
            AugmentedSixthKind::French => (2, 0),
            AugmentedSixthKind::Swiss => (2, 1),
        };
        let rest = figure[name.len()..].trim_start_matches('+');
        // A figure written `6/5` is the same as `65`.
        let rest = unslash_inversion(rest);
        let first = rest.chars().next();
        let rest = if !first.is_some_and(|digit| digit.is_ascii_digit()) {
            format!("{default}{rest}")
        } else if first == Some('6')
            && kind != AugmentedSixthKind::Italian
            && !rest
                .chars()
                .nth(1)
                .is_some_and(|next| next.is_ascii_digit())
        {
            format!("{default}{}", &rest[1..])
        } else {
            rest
        };
        let mut bracketed = Vec::new();
        if kind != AugmentedSixthKind::French {
            bracketed.push((1, 1));
        }
        if matches!(kind, AugmentedSixthKind::French | AugmentedSixthKind::Swiss) {
            bracketed.push((1, 3));
        }
        return Ok(NumeralAlone {
            numeral: name.to_string(),
            rest,
            degree,
            alteration,
            minor: true,
            bracketed,
        });
    }

    let (numeral, rest) = split_roman_prefix(figure)?;
    Ok(NumeralAlone {
        numeral: numeral.to_string(),
        rest: rest.to_string(),
        degree: roman_degree(numeral)?,
        alteration: 0,
        minor: false,
        bracketed: Vec::new(),
    })
}

/// The augmented sixth a figure opens with, if it opens with one.
pub(super) fn augmented_sixth_prefix(figure: &str) -> Option<AugmentedSixthKind> {
    for (name, kind) in [
        ("It", AugmentedSixthKind::Italian),
        ("Ger", AugmentedSixthKind::German),
        ("Fr", AugmentedSixthKind::French),
        ("Sw", AugmentedSixthKind::Swiss),
    ] {
        if figure.starts_with(name) {
            return Some(kind);
        }
    }
    None
}

/// `6/5` written as `65`, which is the same figure with a slash in it.
pub(super) fn unslash_inversion(figure: &str) -> String {
    let letters: Vec<char> = figure.chars().collect();
    let mut out = String::with_capacity(figure.len());
    let mut index = 0;
    while index < letters.len() {
        if letters[index] == '/'
            && index > 0
            && letters[index - 1].is_ascii_digit()
            && letters.get(index + 1).is_some_and(char::is_ascii_digit)
        {
            index += 1;
            continue;
        }
        out.push(letters[index]);
        index += 1;
    }
    out
}

/// Which scale degree a figured-bass column puts in the bass, for a chord
/// whose root stands on `degree`.
///
/// This is music21's `bassScaleDegreeFromNotation`. It works the answer out
/// rather than looking it up: a chord of naturals spaced by the column's own
/// numbers is spelled, its root found, and the bass is that many steps below
/// the root. A column that implies no root at all — `54`, say — leaves the
/// root in the bass, which is what the degree already says.
pub fn bass_scale_degree_from_notation(degree: u8, numbers: &[u8]) -> Result<u8> {
    bass_scale_degree_from_notation_in(degree, numbers, 7)
}

/// The same in a collection of a given size, which for anything but a key is
/// not seven.
pub(super) fn bass_scale_degree_from_notation_in(
    degree: u8,
    numbers: &[u8],
    cardinality: u8,
) -> Result<u8> {
    if !FIGURES_IMPLYING_ROOT.contains(&numbers) {
        return Ok(degree);
    }
    let middle_c = 22;
    let mut pitches = vec![natural_at_diatonic_number(middle_c)?];
    for number in numbers {
        pitches.push(natural_at_diatonic_number(
            middle_c + IntegerType::from(*number) - 1,
        )?);
    }
    let spelled = Chord::new(pitches.as_slice())?;
    let root = spelled
        .root()
        .ok_or_else(|| Error::Chord("figured bass column has no root".to_string()))?;
    let distance = root.diatonic_note_number() - middle_c;
    let count = IntegerType::from(cardinality);
    let bass = (IntegerType::from(degree) - distance).rem_euclid(count);
    Ok(if bass == 0 { cardinality } else { bass as u8 })
}

/// music21's `_setImpliedQualityFromString`: the quality symbol in front of
/// the inversion digits, and the digits left after it.
///
/// The crate's own `dim`, `aug` and `m7b5` spellings are read here too, since
/// a figure written that way is one it has always accepted.
pub(super) fn implied_quality_from_string(
    roman: &str,
    suffix: &str,
    case_matters: bool,
) -> (ImpliedQuality, String) {
    if let Some(rest) = suffix.strip_prefix('o') {
        return (ImpliedQuality::Diminished, rest.to_string());
    }
    if let Some(rest) = suffix.strip_prefix('\u{00f8}') {
        return (ImpliedQuality::HalfDiminished, rest.to_string());
    }
    if let Some(rest) = suffix.strip_prefix('+') {
        return (ImpliedQuality::Augmented, rest.to_string());
    }
    let lower = suffix.to_ascii_lowercase();
    if lower.contains("m7b5") {
        return (ImpliedQuality::HalfDiminished, suffix.to_string());
    }
    if lower.contains("dim") {
        return (ImpliedQuality::Diminished, suffix.to_string());
    }
    if lower.contains("aug") {
        return (ImpliedQuality::Augmented, suffix.to_string());
    }
    // A `d` before the inversion figure is music21's dominant seventh: `Vd7`
    // and `IVd65` are dominant sevenths whatever the scale spells.
    if let Some((leading, figure)) = suffix.rsplit_once('d')
        && matches!(
            figure,
            "7" | "65" | "6/5" | "43" | "4/3" | "42" | "4/2" | "2"
        )
    {
        return (
            ImpliedQuality::DominantSeventh,
            format!("{leading}{figure}"),
        );
    }
    if !case_matters {
        return (ImpliedQuality::Unstated, suffix.to_string());
    }
    let quality = if roman.chars().next().is_some_and(char::is_uppercase) {
        ImpliedQuality::Major
    } else {
        ImpliedQuality::Minor
    };
    (quality, suffix.to_string())
}

/// music21's `sharpen`: raises the alteration in front of a numeral, taking
/// a sharp already written on the root out of the figure so the note is not
/// raised twice.
pub(super) fn sharpen_figure(figure: &mut String) {
    if figure.contains("##") {
        *figure = figure.replace("##8", "#8");
    } else if figure.contains("#2") {
        *figure = figure.replace("#2", "2");
    } else if figure.contains("#4") {
        *figure = figure.replace("#4", "4");
    } else if figure.contains("#6") {
        *figure = figure.replace("#6", "6");
    } else {
        *figure = figure.replace("#8", "");
    }
}

/// Splits the leading roman numeral off a figure, returning the numeral and
/// what follows it. Errors when the value does not begin with one.
pub fn split_roman_prefix(value: &str) -> Result<(&str, &str)> {
    let end = value
        .char_indices()
        .find_map(|(idx, ch)| (!matches!(ch, 'I' | 'V' | 'X' | 'i' | 'v' | 'x')).then_some(idx))
        .unwrap_or(value.len());

    if end == 0 {
        return Err(Error::Chord(format!("No roman numeral found in '{value}'")));
    }

    Ok((&value[..end], &value[end..]))
}

pub(super) fn roman_degree(roman: &str) -> Result<u8> {
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

pub(super) fn suffix_has_seventh(suffix: &str) -> bool {
    let suffix = strip_roman_addition_groups(suffix);
    suffix.contains('7')
        || suffix.contains('9')
        || suffix.contains("11")
        || suffix.contains("13")
        || suffix.contains("65")
        || suffix.contains("43")
        || suffix.contains("42")
}

/// music21's `roman.figureShorthands`, mapping a full figured-bass string to
/// the abbreviation musicians actually write.
///
/// The inversion logic normalizes a figure through this table rather than
/// probing it for substrings, so `642` reads as the third inversion it is and
/// not as the `64` inside it.
pub(super) const FIGURE_SHORTHANDS: [(&str, &str); 20] = [
    ("53", ""),
    ("3", ""),
    ("63", "6"),
    ("753", "7"),
    ("75", "7"),
    ("73", "7"),
    ("9753", "9"),
    ("975", "9"),
    ("953", "9"),
    ("97", "9"),
    ("95", "9"),
    ("93", "9"),
    ("653", "65"),
    ("6b53", "6b5"),
    ("643", "43"),
    ("642", "42"),
    ("bb7b5b3", "o7"),
    ("b7b5b3", "\u{00f8}7"),
    ("bb7b53", "o7"),
    ("b7b53", "\u{00f8}7"),
];

/// Returns the shorthand for a figure, or the figure itself when it has none.
pub(super) fn normalize_figure(figure: &str) -> &str {
    FIGURE_SHORTHANDS
        .iter()
        .find(|(full, _)| *full == figure)
        .map_or(figure, |(_, short)| *short)
}

pub(super) fn parse_inversion(suffix: &str) -> u8 {
    let suffix = strip_roman_addition_groups(suffix);
    // Only the figured-bass digits decide the inversion; quality marks such as
    // `o`, `+` and the half-diminished sign ride along in the suffix.
    let digits: String = suffix.chars().filter(char::is_ascii_digit).collect();

    match normalize_figure(&digits) {
        "42" => 3,
        "43" | "64" => 2,
        "65" | "6" => 1,
        _ => 0,
    }
}

pub(super) fn strip_roman_addition_groups(suffix: &str) -> String {
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

pub(crate) fn degree_to_roman(degree: u8) -> &'static str {
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
