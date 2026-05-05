//! ABC notation format helpers.
//!
//! This module intentionally stays close to reusable ABC token conversion,
//! similar in spirit to `music21.abcFormat`. Complete score layout and
//! application-specific snippets belong in callers.

use crate::{Error, Pitch, Result};

/// Returns an ABC note token for a pitch.
///
/// Octave-less pitches are written in octave 4, matching the crate's default
/// pitch-space behavior.
pub fn abc_note(pitch: &Pitch) -> Result<String> {
    let name = pitch.name();
    let mut chars = name.chars();
    let step = chars
        .next()
        .ok_or_else(|| Error::Pitch("cannot write empty pitch name as ABC".to_string()))?;
    if !matches!(step, 'A'..='G') {
        return Err(Error::Pitch(format!(
            "cannot write pitch step {step:?} as ABC"
        )));
    }

    let accidental = chars
        .map(|modifier| match modifier {
            '#' => Ok('^'),
            '-' => Ok('_'),
            _ => Err(Error::Pitch(format!(
                "cannot write accidental modifier {modifier:?} as ABC"
            ))),
        })
        .collect::<Result<String>>()?;
    let octave = pitch.octave().unwrap_or(4);

    if octave >= 5 {
        Ok(format!(
            "{accidental}{}{}",
            step.to_ascii_lowercase(),
            "'".repeat((octave - 5) as usize)
        ))
    } else {
        Ok(format!(
            "{accidental}{step}{}",
            ",".repeat((4 - octave).max(0) as usize)
        ))
    }
}

/// Returns an ABC rest token.
pub fn abc_rest() -> &'static str {
    "z"
}

/// Returns an ABC chord token for the supplied pitches.
pub fn abc_chord(pitches: &[Pitch]) -> Result<String> {
    let notes = pitches.iter().map(abc_note).collect::<Result<Vec<_>>>()?;
    if notes.is_empty() {
        Ok(abc_rest().to_string())
    } else {
        Ok(format!("[{}]", notes.join("")))
    }
}

/// Returns an ABC duration suffix for a rational note length.
pub fn abc_duration(numerator: u32, denominator: u32) -> Result<String> {
    if denominator == 0 {
        return Err(Error::Music21Object(
            "ABC duration denominator cannot be zero".to_string(),
        ));
    }

    let divisor = gcd(numerator, denominator);
    let top = numerator / divisor;
    let bottom = denominator / divisor;

    if bottom == 1 {
        if top == 1 {
            Ok(String::new())
        } else {
            Ok(top.to_string())
        }
    } else if top == 1 {
        Ok(format!("/{bottom}"))
    } else {
        Ok(format!("{top}/{bottom}"))
    }
}

/// Returns a music21-style pitch name for an ABC note token.
///
/// Rests return `Ok(None)`. Length suffixes are accepted and ignored.
pub fn pitch_name_from_abc_note(token: &str) -> Result<Option<String>> {
    let token = token.trim();
    if token.is_empty() {
        return Err(Error::Pitch("ABC note token cannot be empty".to_string()));
    }

    let token = token
        .strip_prefix('{')
        .and_then(|inner| inner.strip_suffix('}'))
        .unwrap_or(token);

    let mut chars = token.char_indices().peekable();
    let mut accidental = String::new();

    while let Some((_, ch)) = chars.peek().copied() {
        match ch {
            '^' => {
                accidental.push('#');
                chars.next();
            }
            '_' => {
                accidental.push('-');
                chars.next();
            }
            '=' => {
                accidental.clear();
                accidental.push('n');
                chars.next();
            }
            _ => break,
        }
    }

    let Some((_, step)) = chars.next() else {
        return Err(Error::Pitch(format!(
            "ABC note token {token:?} is missing a pitch step"
        )));
    };

    if matches!(step, 'z' | 'Z' | 'x' | 'X') {
        ensure_duration_suffix(token, chars.map(|(_, ch)| ch))?;
        return Ok(None);
    }

    if !matches!(step, 'A'..='G' | 'a'..='g') {
        return Err(Error::Pitch(format!(
            "ABC note token {token:?} has invalid pitch step {step:?}"
        )));
    }

    let mut octave = if step.is_ascii_lowercase() { 5 } else { 4 };
    while let Some((_, ch)) = chars.peek().copied() {
        match ch {
            '\'' => {
                octave += 1;
                chars.next();
            }
            ',' => {
                octave -= 1;
                chars.next();
            }
            _ => break,
        }
    }

    ensure_duration_suffix(token, chars.map(|(_, ch)| ch))?;
    Ok(Some(format!(
        "{}{accidental}{octave}",
        step.to_ascii_uppercase()
    )))
}

/// Returns music21-style pitch names for a simple ABC chord token.
///
/// Length suffixes on the chord or individual notes are accepted and ignored.
pub fn pitch_names_from_abc_chord(token: &str) -> Result<Vec<String>> {
    let token = token.trim();
    let Some(open) = token.find('[') else {
        return match pitch_name_from_abc_note(token)? {
            Some(pitch_name) => Ok(vec![pitch_name]),
            None => Ok(Vec::new()),
        };
    };
    let Some(close_offset) = token[open + 1..].find(']') else {
        return Err(Error::Pitch(format!(
            "ABC chord token {token:?} is missing a closing bracket"
        )));
    };
    let close = open + 1 + close_offset;
    ensure_duration_suffix(token, token[close + 1..].chars())?;

    let mut names = Vec::new();
    let inner = &token[open + 1..close];
    let mut start = 0;
    while start < inner.len() {
        let end = next_abc_note_end(inner, start)?;
        if let Some(name) = pitch_name_from_abc_note(&inner[start..end])? {
            names.push(name);
        }
        start = end;
    }

    Ok(names)
}

fn next_abc_note_end(value: &str, start: usize) -> Result<usize> {
    let mut end = start;
    let mut saw_step = false;

    for (offset, ch) in value[start..].char_indices() {
        let idx = start + offset;
        if !saw_step {
            end = idx + ch.len_utf8();
            match ch {
                '^' | '_' | '=' => {}
                'A'..='G' | 'a'..='g' | 'z' | 'Z' | 'x' | 'X' => saw_step = true,
                _ => {
                    return Err(Error::Pitch(format!(
                        "ABC chord token has invalid note character {ch:?}"
                    )));
                }
            }
            continue;
        }

        if matches!(ch, '\'' | ',' | '/' | '0'..='9') {
            end = idx + ch.len_utf8();
        } else {
            break;
        }
    }

    if saw_step {
        Ok(end)
    } else {
        Err(Error::Pitch(
            "ABC chord token is missing a pitch step".to_string(),
        ))
    }
}

fn ensure_duration_suffix(token: &str, suffix: impl IntoIterator<Item = char>) -> Result<()> {
    let invalid = suffix
        .into_iter()
        .find(|ch| !ch.is_ascii_digit() && *ch != '/');

    if let Some(ch) = invalid {
        Err(Error::Pitch(format!(
            "ABC token {token:?} has unsupported suffix character {ch:?}"
        )))
    } else {
        Ok(())
    }
}

fn gcd(mut left: u32, mut right: u32) -> u32 {
    while right != 0 {
        (left, right) = (right, left % right);
    }
    left
}

#[cfg(test)]
mod tests {
    use super::{
        abc_chord, abc_duration, abc_note, abc_rest, pitch_name_from_abc_note,
        pitch_names_from_abc_chord,
    };
    use crate::{Pitch, Result};

    fn pitches(names: &[&str]) -> Result<Vec<Pitch>> {
        names.iter().map(|name| Pitch::from_name(*name)).collect()
    }

    #[test]
    fn writes_pitch_tokens_with_accidentals_and_octaves() -> Result<()> {
        assert_eq!(abc_note(&Pitch::from_name("C4")?)?, "C");
        assert_eq!(abc_note(&Pitch::from_name("C#4")?)?, "^C");
        assert_eq!(abc_note(&Pitch::from_name("E-4")?)?, "_E");
        assert_eq!(abc_note(&Pitch::from_name("C5")?)?, "c");
        assert_eq!(abc_note(&Pitch::from_name("B3")?)?, "B,");
        Ok(())
    }

    #[test]
    fn writes_chord_and_rest_tokens_without_score_layout() -> Result<()> {
        assert_eq!(abc_rest(), "z");
        assert_eq!(abc_chord(&pitches(&["C4", "E4", "G4"])?)?, "[CEG]");
        assert_eq!(abc_chord(&[])?, "z");
        Ok(())
    }

    #[test]
    fn writes_duration_suffixes() -> Result<()> {
        assert_eq!(abc_duration(1, 1)?, "");
        assert_eq!(abc_duration(4, 1)?, "4");
        assert_eq!(abc_duration(1, 4)?, "/4");
        assert_eq!(abc_duration(4, 10)?, "2/5");
        assert!(abc_duration(1, 0).is_err());
        Ok(())
    }

    #[test]
    fn reads_pitch_names_from_note_tokens() -> Result<()> {
        assert_eq!(pitch_name_from_abc_note("C")?.as_deref(), Some("C4"));
        assert_eq!(pitch_name_from_abc_note("c")?.as_deref(), Some("C5"));
        assert_eq!(pitch_name_from_abc_note("B,,")?.as_deref(), Some("B2"));
        assert_eq!(pitch_name_from_abc_note("c''")?.as_deref(), Some("C7"));
        assert_eq!(pitch_name_from_abc_note("^g2")?.as_deref(), Some("G#5"));
        assert_eq!(pitch_name_from_abc_note("_g''")?.as_deref(), Some("G-7"));
        assert_eq!(pitch_name_from_abc_note("=c")?.as_deref(), Some("Cn5"));
        assert_eq!(pitch_name_from_abc_note("z4")?, None);
        Ok(())
    }

    #[test]
    fn reads_pitch_names_from_chord_tokens() -> Result<()> {
        assert_eq!(
            pitch_names_from_abc_chord("[CEG]4")?,
            vec!["C4", "E4", "G4"]
        );
        assert_eq!(
            pitch_names_from_abc_chord("[^C_Eg']2")?,
            vec!["C#4", "E-4", "G6"]
        );
        Ok(())
    }
}
