//! Small ABC notation helpers.

use std::fmt;

use crate::{Error, IntegerType, Pitch, Result};

/// A clef choice for a compact ABC excerpt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbcClef {
    /// Treble clef.
    Treble,
    /// Bass clef.
    Bass,
}

impl fmt::Display for AbcClef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Treble => f.write_str("treble"),
            Self::Bass => f.write_str("bass"),
        }
    }
}

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

/// Chooses a compact treble or bass clef for the supplied pitches.
///
/// The heuristic favors bass clef when the average MIDI number is below middle
/// C or any pitch is below C3.
pub fn abc_clef_for_pitches(pitches: &[Pitch]) -> AbcClef {
    if pitches.is_empty() {
        return AbcClef::Treble;
    }

    let midi_values = pitches.iter().map(Pitch::midi).collect::<Vec<_>>();
    let total = midi_values.iter().sum::<IntegerType>();
    let average = total as f64 / midi_values.len() as f64;
    let lowest = midi_values.iter().min().copied().unwrap_or(60);

    if average < 60.0 || lowest < 48 {
        AbcClef::Bass
    } else {
        AbcClef::Treble
    }
}

/// Returns an ABC chord token for the supplied pitches.
pub fn abc_chord(pitches: &[Pitch]) -> Result<String> {
    let notes = pitches.iter().map(abc_note).collect::<Result<Vec<_>>>()?;
    if notes.is_empty() {
        Ok("z4".to_string())
    } else {
        Ok(format!("[{}]4", notes.join("")))
    }
}

/// Returns a complete one-bar ABC document for a chord.
pub fn abc_chord_document(pitches: &[Pitch]) -> Result<String> {
    Ok(format!(
        "X:1\nL:1/4\nM:4/4\nK:C clef={}\n{} |]\n",
        abc_clef_for_pitches(pitches),
        abc_chord(pitches)?
    ))
}

/// Returns a complete two-bar ABC document showing one chord resolving to another.
pub fn abc_chord_resolution_document(source: &[Pitch], target: &[Pitch]) -> Result<String> {
    let mut combined = Vec::with_capacity(source.len() + target.len());
    combined.extend_from_slice(source);
    combined.extend_from_slice(target);

    Ok(format!(
        "X:1\nL:1/4\nM:4/4\nK:C clef={}\n{} | {} |]\n",
        abc_clef_for_pitches(&combined),
        abc_chord(source)?,
        abc_chord(target)?
    ))
}

/// Returns an ABC duration suffix for a rational note length.
pub fn abc_duration(numerator: u32, denominator: u32) -> Result<String> {
    if denominator == 0 {
        return Err(Error::Polyrhythm(
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

/// Returns an ABC voice body for one polyrhythm component.
pub fn abc_polyrhythm_voice(component: u32, base: u32) -> Result<String> {
    if component == 0 || base == 0 {
        return Err(Error::Polyrhythm(
            "ABC polyrhythm components must be positive".to_string(),
        ));
    }

    if component == base {
        return Ok(std::iter::repeat_n("B", component as usize)
            .collect::<Vec<_>>()
            .join(" "));
    }

    if component == 1 {
        return Ok(format!("B{base}"));
    }

    if component <= 9 {
        let notes = std::iter::repeat_n("B", component as usize)
            .collect::<Vec<_>>()
            .join(" ");
        return Ok(format!("({component}:{base}:{component}{notes}"));
    }

    let duration = abc_duration(base, component)?;
    Ok((0..component)
        .map(|index| {
            let label = if index == 0 {
                format!("\"^{component}:{base}\"")
            } else {
                String::new()
            };
            format!("{label}B{duration}")
        })
        .collect::<Vec<_>>()
        .join(" "))
}

/// Returns a complete percussion ABC document for a polyrhythm.
pub fn abc_polyrhythm_document(components: &[u32], base: u32) -> Result<String> {
    if components.is_empty() {
        return Err(Error::Polyrhythm(
            "ABC polyrhythm document requires at least one component".to_string(),
        ));
    }

    let mut lines = vec![
        "X:1".to_string(),
        "L:1/4".to_string(),
        format!("M:{base}/4"),
        "K:C clef=perc style=x".to_string(),
    ];

    for (index, component) in components.iter().enumerate() {
        lines.push(format!(
            "V:{} name=\"{}\" clef=perc style=x",
            index + 1,
            component
        ));
        lines.push(format!("{} |]", abc_polyrhythm_voice(*component, base)?));
    }

    Ok(format!("{}\n", lines.join("\n")))
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
        AbcClef, abc_chord, abc_chord_document, abc_chord_resolution_document, abc_duration,
        abc_note, abc_polyrhythm_document, abc_polyrhythm_voice,
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
    fn builds_chord_tokens_and_documents() -> Result<()> {
        let chord = pitches(&["C4", "E4", "G4"])?;

        assert_eq!(abc_chord(&chord)?, "[CEG]4");
        assert_eq!(
            abc_chord_document(&chord)?,
            "X:1\nL:1/4\nM:4/4\nK:C clef=treble\n[CEG]4 |]\n"
        );
        Ok(())
    }

    #[test]
    fn chooses_bass_clef_for_low_material() -> Result<()> {
        let source = pitches(&["G2", "B2", "D3", "F3"])?;
        let target = pitches(&["C3", "E3", "G3"])?;

        assert_eq!(super::abc_clef_for_pitches(&source), AbcClef::Bass);
        assert_eq!(
            abc_chord_resolution_document(&source, &target)?,
            "X:1\nL:1/4\nM:4/4\nK:C clef=bass\n[G,,B,,D,F,]4 | [C,E,G,]4 |]\n"
        );
        Ok(())
    }

    #[test]
    fn writes_polyrhythm_abc_documents() -> Result<()> {
        assert_eq!(abc_duration(4, 10)?, "2/5");
        assert_eq!(abc_polyrhythm_voice(3, 4)?, "(3:4:3B B B");
        assert_eq!(
            abc_polyrhythm_voice(11, 4)?,
            "\"^11:4\"B4/11 B4/11 B4/11 B4/11 B4/11 B4/11 B4/11 B4/11 B4/11 B4/11 B4/11"
        );
        assert_eq!(
            abc_polyrhythm_document(&[2, 3], 4)?,
            "X:1\nL:1/4\nM:4/4\nK:C clef=perc style=x\nV:1 name=\"2\" clef=perc style=x\n(2:4:2B B |]\nV:2 name=\"3\" clef=perc style=x\n(3:4:3B B B |]\n"
        );
        Ok(())
    }
}
