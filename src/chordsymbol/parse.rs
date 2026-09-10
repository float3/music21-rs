//! Reading a figure: the root and bass, the quality and extensions the
//! shorthand spells, and the alterations, additions and omissions written
//! after it.

use super::*;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct Music21PitchModifiers {
    pub(super) base: String,
    pub(super) additions: Vec<String>,
    pub(super) omissions: Vec<String>,
}

pub(super) fn split_music21_pitch_modifiers(value: &str) -> Music21PitchModifiers {
    let Some(start) = find_music21_modifier_start(value) else {
        return Music21PitchModifiers {
            base: value.to_string(),
            ..Music21PitchModifiers::default()
        };
    };

    let mut parts = Music21PitchModifiers {
        base: value[..start].trim_end().to_string(),
        ..Music21PitchModifiers::default()
    };
    let mut cursor = start;
    while cursor < value.len() {
        let Some(marker) = music21_modifier_at(value, cursor) else {
            cursor += value[cursor..]
                .chars()
                .next()
                .map(char::len_utf8)
                .unwrap_or(1);
            continue;
        };
        let content_start = cursor + marker.len();
        let content_end = find_music21_modifier_start(&value[content_start..])
            .map(|relative| content_start + relative)
            .unwrap_or(value.len());
        let tokens = value[content_start..content_end]
            .split(|ch: char| ch == ',' || ch.is_whitespace())
            .filter(|token| !token.trim().is_empty())
            .map(|token| token.trim().to_string());

        match marker {
            "add" => parts.additions.extend(tokens),
            "omit" => parts.omissions.extend(tokens),
            _ => {}
        }
        cursor = content_end;
    }

    parts
}

pub(super) fn find_music21_modifier_start(value: &str) -> Option<usize> {
    value
        .char_indices()
        .find_map(|(idx, _)| music21_modifier_at(value, idx).map(|_| idx))
}

pub(super) fn music21_modifier_at(value: &str, idx: usize) -> Option<&'static str> {
    let rest = value.get(idx..)?;
    let lower = rest.to_ascii_lowercase();
    if lower.starts_with("add") && !matches!(rest.as_bytes().get(3), Some(b'(')) {
        Some("add")
    } else if lower.starts_with("omit") && !matches!(rest.as_bytes().get(4), Some(b'(')) {
        Some("omit")
    } else {
        None
    }
}

/// Whether a token after `add` or `omit` names a pitch, `D` or `A-`, rather
/// than a degree, `9` or `b6`.
pub(super) fn is_pitch_name_token(token: &str) -> bool {
    let mut chars = token.chars();
    chars
        .next()
        .is_some_and(|first| ('A'..='G').contains(&first))
        && chars.all(|c| matches!(c, '#' | '-'))
}

pub(super) fn pitch_name_addition(root: &Pitch, pitch_name: &str) -> Option<ChordAlteration> {
    if !is_pitch_name_token(pitch_name) {
        return None;
    }
    let pitch = Pitch::from_name(pitch_name).ok()?;
    let degree = pitch_name_degree(root, pitch_name)?;
    let actual = ((pitch_class(&pitch) + 12 - pitch_class(root)) % 12) as IntegerType;
    let base = base_semitone_for_degree(degree)?.rem_euclid(12);
    let mut semitones = actual - base;
    while semitones > 6 {
        semitones -= 12;
    }
    while semitones < -6 {
        semitones += 12;
    }

    Some(ChordAlteration::new(degree, semitones))
}

pub(super) fn pitch_name_degree(root: &Pitch, pitch_name: &str) -> Option<u8> {
    if !is_pitch_name_token(pitch_name) {
        return None;
    }
    let pitch = Pitch::from_name(pitch_name).ok()?;
    let generic = (step_num(&pitch) - step_num(root)).rem_euclid(7) + 1;
    Some(match generic as u8 {
        2 => 9,
        4 => 11,
        6 => 13,
        degree => degree,
    })
}

/// Whether the figure names a major seventh over a triad that is not major.
///
/// music21 writes those as `mM7`, `+M9`, `minmaj11`, `augmaj13` and `m#7`.
/// The seventh they name is a semitone above the one the triad's own quality
/// gives — `F#mM7` is `F# A C# E#` where `F#m7` is `F# A C# E` — so it is
/// recorded as an alteration of the seventh. Read on the figure as written,
/// since the `m` and the `M` are the same letter in different cases.
pub(super) fn major_seventh_over_a_lesser_triad(suffix: &str) -> bool {
    // Longest marker first, so `minmaj7` is not read as `m` followed by
    // `inmaj7`.
    let Some(rest) = ["min", "aug", "dim", "m", "+", "o"]
        .into_iter()
        .find_map(|marker| suffix.strip_prefix(marker))
    else {
        return false;
    };
    let Some(rest) = rest
        .strip_prefix("maj")
        .or_else(|| rest.strip_prefix('M'))
        .or_else(|| rest.strip_prefix('#'))
    else {
        return false;
    };
    ["7", "9", "11", "13"]
        .into_iter()
        .any(|degree| rest.starts_with(degree))
}

pub(super) fn add_implicit_music21_alterations(
    suffix: &str,
    alterations: &mut Vec<ChordAlteration>,
) {
    let lower = suffix.to_ascii_lowercase();
    if major_seventh_over_a_lesser_triad(suffix)
        && !alterations.iter().any(|alteration| alteration.degree == 7)
    {
        alterations.push(ChordAlteration::new(7, 1));
    }
    if lower.contains("dim5")
        && !alterations
            .iter()
            .any(|alteration| alteration.degree == 5 && alteration.semitones == -1)
    {
        alterations.push(ChordAlteration::new(5, -1));
    }
    if lower.ends_with("7+")
        && !alterations
            .iter()
            .any(|alteration| alteration.degree == 5 && alteration.semitones == 1)
    {
        alterations.push(ChordAlteration::new(5, 1));
    }
}

pub(super) fn alteration_text(alteration: &ChordAlteration) -> String {
    let sign = if alteration.semitones < 0 { "b" } else { "#" };
    format!(
        "{}{}",
        sign.repeat(alteration.semitones.unsigned_abs() as usize),
        alteration.degree
    )
}

pub(super) fn parse_quality(suffix: &str, alterations: &[ChordAlteration]) -> ChordQuality {
    let lower = suffix.to_ascii_lowercase();
    let has_flat_five = alterations
        .iter()
        .any(|alteration| alteration.degree == 5 && alteration.semitones == -1);

    if suffix.starts_with('\u{00f8}') {
        ChordQuality::HalfDiminished
    } else if lower.contains("sus2") {
        ChordQuality::Suspended2
    } else if lower.contains("sus") {
        ChordQuality::Suspended4
    } else if lower.starts_with("maj") || suffix.starts_with('M') {
        ChordQuality::Major
    } else if lower.starts_with("min") || lower.starts_with('m') {
        if has_flat_five && lower.contains('7') {
            ChordQuality::HalfDiminished
        } else {
            ChordQuality::Minor
        }
    } else if lower.starts_with("dim") || lower.starts_with('o') {
        ChordQuality::Diminished
    } else if lower.starts_with("aug") || lower.starts_with('+') {
        ChordQuality::Augmented
    } else if lower.starts_with('5') || lower.starts_with("power") {
        ChordQuality::Power
    } else if lower.starts_with("pedal") {
        ChordQuality::Pedal
    } else if lower.starts_with("dom")
        || lower.starts_with('7')
        || lower.starts_with('9')
        || lower.starts_with("11")
        || lower.starts_with("13")
    {
        ChordQuality::Dominant
    } else {
        ChordQuality::Major
    }
}

pub(super) fn parse_extensions(suffix: &str, alterations: &[ChordAlteration]) -> Vec<u8> {
    let mut extensions = Vec::new();
    let bytes = suffix.as_bytes();
    let mut idx = 0;
    while idx < bytes.len() {
        let byte = bytes[idx];
        if byte.is_ascii_digit()
            && idx
                .checked_sub(1)
                .is_none_or(|prev| !matches!(bytes[prev] as char, '#' | 'b' | '-'))
        {
            let start = idx;
            while idx < bytes.len() && bytes[idx].is_ascii_digit() {
                idx += 1;
            }
            if let Ok(degree) = suffix[start..idx].parse::<u8>()
                && matches!(degree, 6 | 7 | 9 | 11 | 13)
                && !extensions.contains(&degree)
            {
                extensions.push(degree);
            }
        } else {
            idx += 1;
        }
    }

    for alteration in alterations {
        if alteration.degree > 5 && !extensions.contains(&alteration.degree) {
            extensions.push(alteration.degree);
        }
    }

    extensions.sort_unstable();
    extensions
}

pub(super) fn strip_addition_groups(suffix: &str) -> String {
    let lower = suffix.to_ascii_lowercase();
    let mut stripped = String::with_capacity(suffix.len());
    let mut cursor = 0;

    while let Some(relative_start) = lower[cursor..].find("add(") {
        let start = cursor + relative_start;
        let content_start = start + "add(".len();
        let Some(relative_end) = suffix[content_start..].find(')') else {
            break;
        };

        stripped.push_str(&suffix[cursor..start]);
        cursor = content_start + relative_end + 1;
    }

    stripped.push_str(&suffix[cursor..]);
    stripped
}

pub(super) fn parse_additions(suffix: &str) -> Vec<ChordAlteration> {
    let lower = suffix.to_ascii_lowercase();
    let mut additions = Vec::new();
    let mut cursor = 0;

    while let Some(relative_start) = lower[cursor..].find("add(") {
        let content_start = cursor + relative_start + "add(".len();
        let Some(relative_end) = suffix[content_start..].find(')') else {
            break;
        };
        let content_end = content_start + relative_end;

        for token in
            suffix[content_start..content_end].split(|ch: char| ch == ',' || ch.is_whitespace())
        {
            if let Some(addition) = parse_addition_token(token) {
                additions.push(addition);
            }
        }

        cursor = content_end + 1;
    }

    additions
}

pub(super) fn parse_omissions(suffix: &str) -> Vec<u8> {
    let lower = suffix.to_ascii_lowercase();
    let bytes = lower.as_bytes();
    let mut omissions = Vec::new();
    let mut cursor = 0;

    while cursor < bytes.len() {
        let marker_len = if bytes[cursor..].starts_with(b"omit") {
            4
        } else if bytes[cursor..].starts_with(b"no") {
            2
        } else {
            cursor += 1;
            continue;
        };

        cursor += marker_len;
        while cursor < bytes.len() && (bytes[cursor].is_ascii_whitespace() || bytes[cursor] == b'(')
        {
            cursor += 1;
        }

        let degree_start = cursor;
        while cursor < bytes.len() && bytes[cursor].is_ascii_digit() {
            cursor += 1;
        }
        if degree_start == cursor {
            continue;
        }

        if let Ok(degree) = std::str::from_utf8(&bytes[degree_start..cursor])
            .unwrap_or_default()
            .parse::<u8>()
            && !omissions.contains(&degree)
        {
            omissions.push(degree);
        }
    }

    omissions
}

pub(super) fn parse_addition_token(token: &str) -> Option<ChordAlteration> {
    let token = token.trim();
    if token.is_empty() {
        return None;
    }

    let (semitones, degree) = match token.as_bytes()[0] as char {
        '#' => (1, &token[1..]),
        'b' | '-' => (-1, &token[1..]),
        _ => (0, token),
    };

    degree
        .parse::<u8>()
        .ok()
        .map(|degree| ChordAlteration::new(degree, semitones))
}

pub(super) fn parse_alterations(suffix: &str) -> Vec<ChordAlteration> {
    let bytes = suffix.as_bytes();
    let mut alterations = Vec::new();
    let mut idx = 0;
    while idx < bytes.len() {
        let semitones = match bytes[idx] as char {
            '#' => 1,
            'b' | '-' => -1,
            _ => {
                idx += 1;
                continue;
            }
        };
        idx += 1;
        let start = idx;
        while idx < bytes.len() && bytes[idx].is_ascii_digit() {
            idx += 1;
        }
        if start == idx {
            continue;
        }
        if let Ok(degree) = suffix[start..idx].parse::<u8>() {
            alterations.push(ChordAlteration::new(degree, semitones));
        }
    }
    alterations
}

pub(super) fn parse_pitch_only(value: &str) -> Result<Pitch> {
    let (name, rest) = parse_pitch_prefix(value)?;
    if !rest.is_empty() {
        return Err(Error::Chord(format!("invalid slash bass {value:?}")));
    }
    Pitch::from_name(name)
}

pub(super) fn parse_pitch_prefix(value: &str) -> Result<(String, &str)> {
    let mut chars = value.char_indices();
    let Some((_, first)) = chars.next() else {
        return Err(Error::Chord("missing pitch name".to_string()));
    };

    if !matches!(first.to_ascii_uppercase(), 'A'..='G') {
        return Err(Error::Chord(format!("invalid pitch name in {value:?}")));
    }

    let mut end = first.len_utf8();
    let mut name = first.to_ascii_uppercase().to_string();
    for (idx, ch) in chars {
        match ch {
            '#' => {
                name.push('#');
                end = idx + ch.len_utf8();
            }
            'b' | '-' => {
                name.push('-');
                end = idx + ch.len_utf8();
            }
            _ => break,
        }
    }

    Ok((name, &value[end..]))
}

pub(super) fn default_extension_interval(degree: u8) -> &'static str {
    match degree {
        6 => "M6",
        9 => "M9",
        11 => "P11",
        13 => "M13",
        _ => "P1",
    }
}

pub(super) fn altered_interval(alteration: &ChordAlteration) -> Result<(u8, &'static str)> {
    match (alteration.degree, alteration.semitones) {
        (5, -1) => Ok((5, "d5")),
        (5, 1) => Ok((5, "a5")),
        // The seventh a figure raises is the major seventh, since the one it
        // is raised from is the minor seventh every non-major kind carries.
        (7, -1) => Ok((7, "d7")),
        (7, 1) => Ok((7, "M7")),
        (9, -1) => Ok((9, "m9")),
        (9, 1) => Ok((9, "a9")),
        (11, 1) => Ok((11, "a11")),
        (13, -1) => Ok((13, "m13")),
        (13, 1) => Ok((13, "a13")),
        _ => Err(Error::Chord(format!(
            "unsupported chord-symbol alteration {alteration:?}"
        ))),
    }
}

pub(super) fn added_interval(addition: &ChordAlteration) -> Result<(u8, &'static str)> {
    let degree = match addition.degree {
        2 => 9,
        4 => 11,
        6 => 13,
        degree => degree,
    };

    match (degree, addition.semitones) {
        (3, -1) => Ok((degree, "m3")),
        (3, 0) => Ok((degree, "M3")),
        (3, 1) => Ok((degree, "a3")),
        (5, -1) => Ok((degree, "d5")),
        (5, 0) => Ok((degree, "P5")),
        (5, 1) => Ok((degree, "a5")),
        (7, -1) => Ok((degree, "m7")),
        (7, 0) => Ok((degree, "M7")),
        (9, -1) => Ok((degree, "m9")),
        (9, 0) => Ok((degree, "M9")),
        (9, 1) => Ok((degree, "a9")),
        (11, -1) => Ok((degree, "d11")),
        (11, 0) => Ok((degree, "P11")),
        (11, 1) => Ok((degree, "a11")),
        (13, -1) => Ok((degree, "m13")),
        (13, 0) => Ok((degree, "M13")),
        (13, 1) => Ok((degree, "a13")),
        _ => Err(Error::Chord(format!(
            "unsupported chord-symbol added tone {addition:?}"
        ))),
    }
}
