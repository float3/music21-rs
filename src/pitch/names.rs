//! What a pitch is called: its German, Italian, French and Spanish names,
//! its full name, and its name in Unicode.

use super::*;

impl Pitch {
    /// Returns the German name, where `B` is `H`, `B-` is `B`, sharps add
    /// `is` and flats add `es` or `s`. Errors on a microtonal accidental.
    pub fn german(&self) -> Result<String> {
        let alter = self.whole_alteration("german")?;
        let mut step = self.step.as_char().to_string();
        let mut alter = alter;
        if self.step == StepName::B {
            if alter == -1 {
                alter = 0;
            } else {
                step = "H".to_string();
            }
        }
        Ok(match alter {
            0 => step,
            alter if alter > 0 => step + &"is".repeat(alter as usize),
            alter => {
                let first = if matches!(step.as_str(), "C" | "D" | "F" | "G" | "H") {
                    "es"
                } else {
                    "s"
                };
                step + first + &"es".repeat(alter.unsigned_abs() as usize - 1)
            }
        })
    }

    /// Returns the Italian solfège name, such as `"do diesis"` or
    /// `"si doppio bemolle"`. Errors on a microtonal accidental or more than
    /// four sharps or flats.
    pub fn italian(&self) -> Result<String> {
        let alter = self.whole_alteration("italian")?;
        let solfege = match self.step {
            StepName::C => "do",
            StepName::D => "re",
            StepName::E => "mi",
            StepName::F => "fa",
            StepName::G => "sol",
            StepName::A => "la",
            StepName::B => "si",
        };
        let cardinality = match alter.unsigned_abs() {
            0 => return Ok(solfege.to_string()),
            1 => " ",
            2 => " doppio ",
            3 => " triplo ",
            4 => " quadruplo ",
            _ => {
                return Err(Error::Pitch(format!(
                    "entirely too many accidentals for an Italian name: {self}"
                )));
            }
        };
        let kind = if alter > 0 { "diesis" } else { "bemolle" };
        Ok(format!("{solfege}{cardinality}{kind}"))
    }

    /// Returns the French solfège name, such as `"ré bémol"` or
    /// `"fa double dièse"`. Errors on a microtonal accidental or more than
    /// four sharps or flats.
    pub fn french(&self) -> Result<String> {
        let alter = self.whole_alteration("french")?;
        let solfege = match self.step {
            StepName::D => "ré",
            other => romance_solfege(other),
        };
        let multiplier = match alter.unsigned_abs() {
            0 => return Ok(solfege.to_string()),
            1 => "",
            2 => " double",
            3 => " triple",
            4 => " quadruple",
            _ => {
                return Err(Error::Pitch(format!(
                    "entirely too many accidentals for a French name: {self}"
                )));
            }
        };
        let kind = if alter > 0 { "dièse" } else { "bémol" };
        Ok(format!("{solfege}{multiplier} {kind}"))
    }

    /// Returns the Spanish solfège name, such as `"re bemol"` or
    /// `"fa doble sostenido"`. Errors on a microtonal accidental or more than
    /// four sharps or flats.
    pub fn spanish(&self) -> Result<String> {
        let alter = self.whole_alteration("spanish")?;
        let solfege = romance_solfege(self.step);
        let multiplier = match alter.unsigned_abs() {
            0 => return Ok(solfege.to_string()),
            1 => "",
            2 => " doble",
            3 => " triple",
            4 => " cuádruple",
            _ => {
                return Err(Error::Pitch(format!(
                    "entirely too many accidentals for a Spanish name: {self}"
                )));
            }
        };
        let kind = if alter > 0 { "sostenido" } else { "bemol" };
        Ok(format!("{solfege}{multiplier} {kind}"))
    }

    /// Returns the name with the accidental as a Unicode symbol, such as
    /// `"C♯"` or `"G𝄫"`.
    pub fn unicode_name(&self) -> String {
        if self.accidental.alter() == 0.0 {
            return self.step.as_char().to_string();
        }
        format!("{}{}", self.step.as_char(), self.accidental.unicode())
    }

    /// Returns music21's `fullName`: the step, the accidental's full name, the
    /// octave and any microtone, as in `E-flat in octave 4 (+20c)`.
    pub fn full_name(&self) -> String {
        let mut name = self.step.as_char().to_string();
        // music21 asks whether the pitch carries an accidental object, not
        // whether that accidental alters anything: a written natural is
        // named, and a bare `C` — which carries none — is not.
        if self.has_accidental {
            name.push('-');
            name.push_str(self.accidental.full_name());
        }
        if let Some(octave) = self.octave {
            name.push_str(&format!(" in octave {octave}"));
        }
        if let Some(microtone) = &self.microtone
            && microtone.cents() != 0.0
        {
            name.push(' ');
            name.push_str(&microtone.to_string());
        }
        name
    }

    /// Returns the name with its octave and, when it has one that is not
    /// zero, its microtone: music21's `str(Pitch)`, `A4(+20c)`.
    pub fn name_with_octave_and_microtone(&self) -> String {
        match &self.microtone {
            Some(microtone) if microtone.cents() != 0.0 => {
                format!("{}{microtone}", self.name_with_octave())
            }
            _ => self.name_with_octave(),
        }
    }

    /// Returns [`Self::unicode_name`] followed by the octave when one is set.
    pub fn unicode_name_with_octave(&self) -> String {
        match self.octave {
            Some(octave) => format!("{}{octave}", self.unicode_name()),
            None => self.unicode_name(),
        }
    }

    pub(super) fn whole_alteration(&self, language: &str) -> Result<IntegerType> {
        let alter = self.accidental.alter();
        if alter.fract() != 0.0 {
            return Err(Error::Pitch(match language {
                "german" => {
                    "Es geht nicht \"german\" zu benutzen mit Microtönen.  Schade!".to_string()
                }
                "italian" => "Non si puo usare `italian` con microtoni".to_string(),
                "french" => {
                    "On ne peut pas utiliser les microtones avec \"french.\" Quelle Dommage!"
                        .to_string()
                }
                "spanish" => "Unsupported accidental type.".to_string(),
                other => {
                    format!("{other} names cannot express the microtonal accidental of {self}")
                }
            }));
        }
        Ok(alter as IntegerType)
    }
}

/// Canonical pitch names for chromatic pitch classes.
pub const CHROMATIC_PITCH_CLASS_NAMES: [&str; 12] = [
    "C", "D-", "D", "E-", "E", "F", "F#", "G", "A-", "A", "B-", "B",
];

/// Returns a canonical pitch name for a chromatic pitch class.
pub fn pitch_class_name(pitch_class: u8) -> &'static str {
    CHROMATIC_PITCH_CLASS_NAMES[pitch_class as usize % 12]
}

pub(super) fn romance_solfege(step: StepName) -> &'static str {
    match step {
        StepName::C => "do",
        StepName::D => "re",
        StepName::E => "mi",
        StepName::F => "fa",
        StepName::G => "sol",
        StepName::A => "la",
        StepName::B => "si",
    }
}
