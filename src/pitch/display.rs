//! Whether a written accidental is needed: music21's
//! `updateAccidentalDisplay`, with its rules for repeats, octaves, ties,
//! simultaneities and key signatures.

use super::*;

impl Pitch {
    /// music21's `_nameInKeySignature`: whether one of a key signature's
    /// altered pitches has this pitch's step and the same accidental. A
    /// pitch with no accidental object never matches.
    pub fn name_in_key_signature(&self, altered_pitches: &[Pitch]) -> bool {
        let Some(name) = self.accidental_name() else {
            return false;
        };
        altered_pitches
            .iter()
            .any(|p| p.step == self.step && p.accidental_name() == Some(name))
    }

    /// music21's `_stepInKeySignature`: whether a key signature alters this
    /// pitch's step at all, whatever the accidental.
    pub fn step_in_key_signature(&self, altered_pitches: &[Pitch]) -> bool {
        altered_pitches.iter().any(|p| p.step == self.step)
    }

    pub(super) fn set_display_status_creating(&mut self, status: bool) {
        self.accidental
            .get_or_insert_with(Accidental::natural)
            .set_display_status(Some(status));
    }

    /// Records whether the accidental is shown, where there is one to show.
    fn set_display_status(&mut self, status: bool) {
        if let Some(accidental) = &mut self.accidental {
            accidental.set_display_status(Some(status));
        }
    }

    fn accidental_name(&self) -> Option<&str> {
        self.accidental.as_ref().map(Accidental::name)
    }

    fn accidental_display_type(&self) -> Option<&str> {
        self.accidental.as_ref().map(Accidental::display_type)
    }

    fn accidental_display_status(&self) -> Option<bool> {
        self.accidental
            .as_ref()
            .and_then(Accidental::display_status)
    }

    /// Decides whether this pitch's accidental should be shown, given the
    /// pitches before it, and records the answer as the accidental's
    /// `display_status`, adding a natural where a cautionary one is called
    /// for: music21's `updateAccidentalDisplay`, with the same rules for
    /// repeats, octaves, ties, key signatures and simultaneities.
    pub fn update_accidental_display(&mut self, options: &AccidentalDisplayOptions<'_>) {
        let altered = options.altered_pitches;
        let past_all: Vec<&Pitch> = options
            .pitch_past_measure
            .iter()
            .chain(options.pitch_past.iter())
            .collect();
        let mut display_if_no_previous_accidentals = false;

        if !options.override_status && self.accidental_display_status().is_some() {
            return;
        }
        if self.accidental_display_type() == Some("never") {
            self.set_display_status(false);
            return;
        }
        if options.last_note_was_tied {
            let even_tied = self.accidental_display_type() == Some("even-tied");
            self.set_display_status(even_tied);
            return;
        }
        if options.cautionary_pitch_class
            && options
                .other_simultaneous_pitches
                .iter()
                .any(|p| p.step == self.step && p.pitch_class() != self.pitch_class())
        {
            self.set_display_status_creating(true);
            return;
        }
        if options.cautionary_all
            || matches!(self.accidental_display_type(), Some("even-tied" | "always"))
        {
            self.set_display_status_creating(true);
            return;
        }
        if past_all.is_empty() {
            if self.accidental.is_some()
                && (options.override_status || self.accidental_display_status() != Some(true))
            {
                let status = if self.accidental_name() == Some("natural") {
                    self.step_in_key_signature(altered)
                } else {
                    !self.name_in_key_signature(altered)
                };
                self.set_display_status(status);
            } else if self.accidental_display_status() == Some(true)
                && self.name_in_key_signature(altered)
            {
                self.set_display_status(false);
            } else if matches!(self.accidental_name(), None | Some("natural"))
                && self.step_in_key_signature(altered)
            {
                self.set_display_status_creating(true);
            }
            return;
        }
        for past in options.pitch_past.iter().rev() {
            if past.step == self.step && past.octave == self.octave {
                if past.name() != self.name() {
                    self.set_display_status_creating(true);
                    return;
                }
                break;
            }
        }
        let mut set_from_pitch_past = false;
        let out_of_measure_length = options.pitch_past_measure.len();
        let self_name = self.name();
        let self_name_with_octave = self.name_with_octave();
        let name_in_key = self.name_in_key_signature(altered);
        let step_in_key = self.step_in_key_signature(altered);
        let if_absolutely_necessary =
            self.accidental_display_type() == Some("if-absolutely-necessary");
        for i in (0..past_all.len()).rev() {
            let past_in_measure = i >= out_of_measure_length;
            let continuous_repeats_in_measure = past_in_measure
                && past_all[i..]
                    .iter()
                    .all(|p| p.name_with_octave() == self_name_with_octave);
            if !past_in_measure && if_absolutely_necessary {
                break;
            }
            if !past_in_measure
                && self.accidental.is_some()
                && self.accidental_name() != Some("natural")
                && !name_in_key
            {
                self.set_display_status(true);
                return;
            }
            let past = past_all[i];
            if past.step != self.step {
                continue;
            }
            let octave_match = self.octave == past.octave;
            let past_acc = past.written_accidental();
            let past_name = past_acc.map(Accidental::name);
            let past_status = past_acc.and_then(Accidental::display_status);
            let self_acc_name = self.accidental_name().map(str::to_string);
            let self_status = self.accidental_display_status();

            if continuous_repeats_in_measure && past_status == Some(true) {
                self.set_display_status(false);
                return;
            } else if continuous_repeats_in_measure
                && past_acc.is_some()
                && self_acc_name.is_some()
                && past_name == self_acc_name.as_deref()
            {
                if !name_in_key && (!octave_match || past_status == Some(false)) {
                    display_if_no_previous_accidentals = true;
                    continue;
                }
                self.set_display_status(false);
                set_from_pitch_past = true;
                break;
            } else if past_name == Some("natural")
                && (self_acc_name.is_none() || self_acc_name.as_deref() == Some("natural"))
            {
                if continuous_repeats_in_measure {
                    if step_in_key && !octave_match {
                        self.set_display_status_creating(true);
                    } else {
                        self.set_display_status(false);
                    }
                } else if step_in_key
                    && (options.cautionary_not_immediate_repeat || !past_in_measure)
                {
                    self.set_display_status_creating(true);
                } else {
                    self.set_display_status(false);
                }
                set_from_pitch_past = true;
                break;
            } else if past_acc.is_some()
                && past.name() != self_name
                && past_name != Some("natural")
                && (self_acc_name.is_none() || self_status == Some(false))
            {
                if !octave_match && !options.cautionary_pitch_class {
                    continue;
                }
                if !octave_match && if_absolutely_necessary {
                    continue;
                }
                self.set_display_status_creating(true);
                set_from_pitch_past = true;
                break;
            } else if self_acc_name.is_some()
                && (((past_acc.is_none() || past_name == Some("natural"))
                    && self_acc_name.as_deref() != Some("natural"))
                    || (past_acc.is_some()
                        && past_name != self_acc_name.as_deref()
                        && (octave_match || !if_absolutely_necessary)))
            {
                self.set_display_status(true);
                set_from_pitch_past = true;
                break;
            } else if past_acc.is_none() && self_acc_name.is_some() {
                let status = if self_acc_name.as_deref() == Some("natural") {
                    step_in_key
                } else {
                    true
                };
                self.set_display_status(status);
                set_from_pitch_past = true;
                break;
            } else if !continuous_repeats_in_measure
                && past_acc.is_some()
                && self_acc_name.is_some()
                && past_name == self_acc_name.as_deref()
                && octave_match
            {
                if !options.cautionary_not_immediate_repeat && past_status != Some(false) {
                    self.set_display_status(false);
                    display_if_no_previous_accidentals = false;
                    set_from_pitch_past = true;
                    break;
                } else if past_status == Some(false) {
                    display_if_no_previous_accidentals = true;
                } else {
                    self.set_display_status(!name_in_key);
                    return;
                }
            }
        }
        if display_if_no_previous_accidentals {
            if !name_in_key {
                self.set_display_status_creating(true);
            } else {
                self.set_display_status(false);
            }
        } else if !set_from_pitch_past && self.accidental.is_some() {
            let status = if self.accidental_name() == Some("natural") {
                step_in_key
            } else {
                !name_in_key
            };
            self.set_display_status(status);
        } else if !set_from_pitch_past && step_in_key {
            self.set_display_status_creating(true);
        }
    }
}

/// The context music21's `updateAccidentalDisplay` reads: the pitches that
/// came before, in this measure and the previous one, the other pitches
/// sounding at the same time, the key signature's altered pitches, and the
/// cautionary-accidental switches, with music21's defaults.
#[derive(Clone, Debug)]
pub struct AccidentalDisplayOptions<'a> {
    /// Pitches preceding this one in the same measure.
    pub pitch_past: &'a [Pitch],
    /// Pitches preceding this one in the previous measure.
    pub pitch_past_measure: &'a [Pitch],
    /// Other pitches in the same simultaneity.
    pub other_simultaneous_pitches: &'a [Pitch],
    /// The key signature's altered pitches.
    pub altered_pitches: &'a [Pitch],
    /// Whether a past accidental in any octave calls for a cautionary one.
    pub cautionary_pitch_class: bool,
    /// Whether every accidental is shown.
    pub cautionary_all: bool,
    /// Whether an already decided `display_status` is decided again.
    pub override_status: bool,
    /// Whether an altered pitch shows its accidental again unless it
    /// immediately repeats.
    pub cautionary_not_immediate_repeat: bool,
    /// Whether this pitch follows a tie.
    pub last_note_was_tied: bool,
}

impl Default for AccidentalDisplayOptions<'_> {
    fn default() -> Self {
        Self {
            pitch_past: &[],
            pitch_past_measure: &[],
            other_simultaneous_pitches: &[],
            altered_pitches: &[],
            cautionary_pitch_class: true,
            cautionary_all: false,
            override_status: false,
            cautionary_not_immediate_repeat: true,
            last_note_was_tied: false,
        }
    }
}
