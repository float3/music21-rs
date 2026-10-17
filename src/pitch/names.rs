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
        if self.accidental().alter() == 0.0 {
            return self.step.as_char().to_string();
        }
        format!("{}{}", self.step.as_char(), self.accidental().unicode())
    }

    /// Returns music21's `fullName`: the step, the accidental's full name, the
    /// octave and any microtone, as in `E-flat in octave 4 (+20c)`.
    pub fn full_name(&self) -> String {
        let mut name = self.step.as_char().to_string();
        // music21 asks whether the pitch carries an accidental object, not
        // whether that accidental alters anything: a written natural is
        // named, and a bare `C` — which carries none — is not.
        if let Some(accidental) = &self.accidental {
            name.push('-');
            name.push_str(accidental.full_name());
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
        let alter = self.accidental().alter();
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
    "C", "Db", "D", "Eb", "E", "F", "F#", "G", "Ab", "A", "Bb", "B",
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

/// One of the languages a pitch has a name in: music21's
/// `naturalLanguageObjects.SUPPORTED_LANGUAGES`.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Language {
    /// German, where `B` is `H` and `B-` is `B`.
    German,
    /// French, whose names are solfège.
    French,
    /// Italian, whose names are solfège.
    Italian,
    /// Spanish, whose names are solfège.
    Spanish,
}

impl Language {
    /// Every one of them, in music21's order.
    pub const ALL: [Language; 4] = [Self::German, Self::French, Self::Italian, Self::Spanish];

    /// The language music21 names with this code -- `de`, `fr`, `it`, `es`
    /// -- or nothing for a code it does not support.
    #[must_use]
    pub fn from_code(code: &str) -> Option<Self> {
        match code {
            "de" => Some(Self::German),
            "fr" => Some(Self::French),
            "it" => Some(Self::Italian),
            "es" => Some(Self::Spanish),
            _ => None,
        }
    }

    /// music21's code for it.
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Self::German => "de",
            Self::French => "fr",
            Self::Italian => "it",
            Self::Spanish => "es",
        }
    }

    /// What a pitch is called in this language.
    fn name_of(self, pitch: &Pitch) -> Result<String> {
        match self {
            Self::German => pitch.german(),
            Self::French => pitch.french(),
            Self::Italian => pitch.italian(),
            Self::Spanish => pitch.spanish(),
        }
    }
}

/// Every name a pitch has in `language`, each with the name this crate
/// spells it by: music21's `generateLanguageDictionary`.
///
/// The names are read off the pitches themselves, as music21 reads them --
/// the seven letters, each with up to four flats and four sharps, written
/// in the language and paired with what was written -- so there is one
/// table and not two. A name two pitches share is the later of them, which
/// is what music21's dictionary keeps.
///
/// ```
/// use music21_rs::pitch::{Language, language_dictionary};
///
/// let german = language_dictionary(Language::German);
/// assert_eq!(german.len(), 63);
/// assert!(german.contains(&("H".to_string(), "B".to_string())));
/// assert!(german.contains(&("Es".to_string(), "E-".to_string())));
/// ```
#[must_use]
pub fn language_dictionary(language: Language) -> Vec<(String, String)> {
    let mut named: Vec<(String, String)> = Vec::new();
    for step in ['A', 'B', 'C', 'D', 'E', 'F', 'G'] {
        for accidental in ["----", "---", "--", "-", "", "#", "##", "###", "####"] {
            let spelled = format!("{step}{accidental}");
            let Ok(pitch) = Pitch::from_name(&spelled) else {
                continue;
            };
            let Ok(name) = language.name_of(&pitch) else {
                continue;
            };
            match named.iter_mut().find(|(known, _)| *known == name) {
                // A name two pitches share is the later of them, as
                // music21's dictionary is written over.
                Some((_, kept)) => *kept = spelled,
                None => named.push((name, spelled)),
            }
        }
    }
    named
}

/// The pitch a name in another language means, or a C natural where that
/// language calls no pitch by that name: music21's `toPitch`.
///
/// ```
/// use music21_rs::pitch::{Language, from_language_name};
///
/// assert_eq!(from_language_name("Es", Language::German).name(), "Eb");
/// assert_eq!(from_language_name("H", Language::German).name(), "B");
/// // A name the language does not use is a C, as music21 answers.
/// assert_eq!(from_language_name("Es", Language::Italian).name(), "C");
/// ```
pub fn from_language_name(name: &str, language: Language) -> Pitch {
    language_dictionary(language)
        .into_iter()
        .find(|(known, _)| known == name)
        .and_then(|(_, spelled)| Pitch::from_name(&spelled).ok())
        .unwrap_or_else(|| Pitch::from_name("C").expect("a C natural is a pitch"))
}
