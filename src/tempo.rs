//! Metronome marks, a port of the stream-free part of music21's `tempo`.

use crate::{
    defaults::FloatType,
    duration::Duration,
    error::{Error, Result},
    stream::Stream,
};

/// The tempo words music21 knows and the beats per minute each implies,
/// in music21's order.
pub const DEFAULT_TEMPO_VALUES: [(&str, FloatType); 30] = [
    ("larghissimo", 16.0),
    ("largamente", 32.0),
    ("grave", 40.0),
    ("molto adagio", 40.0),
    ("largo", 46.0),
    ("lento", 52.0),
    ("adagio", 56.0),
    ("slow", 56.0),
    ("langsam", 56.0),
    ("larghetto", 60.0),
    ("adagietto", 66.0),
    ("andante", 72.0),
    ("andantino", 80.0),
    ("andante moderato", 83.0),
    ("maestoso", 88.0),
    ("moderato", 92.0),
    ("moderate", 92.0),
    ("allegretto", 108.0),
    ("animato", 120.0),
    ("allegro moderato", 128.0),
    ("allegro", 132.0),
    ("fast", 132.0),
    ("schnell", 132.0),
    ("allegrissimo", 140.0),
    ("molto allegro", 144.0),
    ("très vite", 144.0),
    ("vivace", 160.0),
    ("vivacissimo", 168.0),
    ("presto", 184.0),
    ("prestissimo", 208.0),
];

/// Converts a tempo counted in one note value into the same tempo counted in
/// another, both given in quarter lengths.
///
/// Sixty half notes a minute is a hundred and twenty quarters.
pub fn convert_tempo_by_referent(
    number: FloatType,
    source_quarter_length: FloatType,
    destination_quarter_length: FloatType,
) -> FloatType {
    let seconds_per_source_beat = 60.0 / number;
    let seconds_per_quarter = seconds_per_source_beat / source_quarter_length;
    60.0 / (seconds_per_quarter * destination_quarter_length)
}

/// Returns the tempo word music21 pairs with a beats-per-minute value, when
/// one lies within two beats of it. Ties go to the lower value, then the
/// alphabetically earlier word, as music21 sorts them.
pub fn default_text_for_number(number: FloatType) -> Option<&'static str> {
    let mut sorted = DEFAULT_TEMPO_VALUES;
    sorted.sort_by(|left, right| {
        left.1
            .partial_cmp(&right.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.0.cmp(right.0))
    });
    sorted
        .iter()
        .find(|(_, value)| (value - 2.0..=value + 2.0).contains(&number))
        .map(|(text, _)| *text)
}

/// Returns the beats per minute music21 pairs with a tempo word, matching the
/// whole text case-insensitively first and any single word of it second.
pub fn default_number_for_text(text: &str) -> Option<FloatType> {
    let lowered = text.to_lowercase();
    let lookup = |candidate: &str| {
        DEFAULT_TEMPO_VALUES
            .iter()
            .find(|(name, _)| *name == candidate)
            .map(|(_, value)| *value)
    };
    lookup(&lowered).or_else(|| lookup(text)).or_else(|| {
        text.split(' ')
            .filter_map(|word| lookup(&word.to_lowercase()))
            .next_back()
    })
}

/// A metronome marking: a beats-per-minute number, a tempo word, and the
/// note value the number counts.
///
/// Either half may be implied from the other: a number alone picks up the
/// nearest tempo word, and a word alone picks up its conventional number.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[must_use]
pub struct MetronomeMark {
    number: Option<FloatType>,
    text: Option<String>,
    referent: Duration,
    number_implicit: bool,
    text_implicit: bool,
    /// What the mark is played at, where that is not what it says.
    #[cfg_attr(feature = "serde", serde(default))]
    number_sounding: Option<FloatType>,
}

impl Default for MetronomeMark {
    /// A mark that says nothing yet: no number, no word, counting quarters.
    /// music21 builds one of these and fills it in.
    fn default() -> Self {
        Self::build(None, None, Duration::quarter())
    }
}

impl MetronomeMark {
    /// A mark of `number` beats per minute, counting quarter notes, with the
    /// tempo word implied from the number when one is close enough.
    pub fn new(number: FloatType) -> Self {
        Self::build(Some(number), None, Duration::quarter())
    }

    /// A mark from a tempo word alone, with the number implied from the word
    /// when music21 knows it.
    pub fn from_text(text: impl Into<String>) -> Self {
        Self::build(None, Some(text.into()), Duration::quarter())
    }

    /// A mark carrying both an explicit number and an explicit word.
    pub fn with_number_and_text(number: FloatType, text: impl Into<String>) -> Self {
        Self::build(Some(number), Some(text.into()), Duration::quarter())
    }

    /// Changes the note value the number counts, so `Duration::half()` makes
    /// the number a count of half notes.
    pub fn with_referent(mut self, referent: Duration) -> Self {
        self.referent = referent;
        self
    }

    fn build(number: Option<FloatType>, text: Option<String>, referent: Duration) -> Self {
        let number_implicit = number.is_none();
        let text_implicit = text.is_none();
        let number = number.or_else(|| text.as_deref().and_then(default_number_for_text));
        let text = text.or_else(|| number.and_then(default_text_for_number).map(String::from));
        Self {
            number_implicit: number_implicit && number.is_some(),
            text_implicit: text_implicit && text.is_some(),
            number,
            text,
            referent,
            number_sounding: None,
        }
    }

    /// The same tempo counted in a different note value: music21's
    /// `getEquivalentByReferent`, so quarter = 60 becomes eighth = 120. The
    /// tempo word is carried over unchanged, implied or not.
    pub fn equivalent_by_referent(&self, referent: Duration) -> MetronomeMark {
        let number = self.number.map(|number| {
            convert_tempo_by_referent(
                number,
                self.referent.quarter_length(),
                referent.quarter_length(),
            )
        });
        Self::build(number, self.text.clone(), referent)
    }

    /// The same number counted in a different note value, so the tempo
    /// itself changes: music21's `getMaintainedNumberWithReferent`.
    pub fn maintained_number_with_referent(&self, referent: Duration) -> MetronomeMark {
        Self::build(self.number, self.text.clone(), referent)
    }

    /// The beats per minute, if known.
    pub fn number(&self) -> Option<FloatType> {
        self.number
    }

    /// Sets the beats per minute, which is then no longer implied. A mark
    /// with no word of its own picks one up, as music21 does.
    pub fn set_number(&mut self, number: Option<FloatType>) {
        self.number = number;
        self.number_implicit = false;
        if self.text.is_none()
            && let Some(number) = number
            && let Some(text) = default_text_for_number(number)
        {
            self.text = Some(text.to_string());
            self.text_implicit = true;
        }
    }

    /// Sets the tempo word, which is then no longer implied. A mark with no
    /// number of its own picks one up.
    pub fn set_text(&mut self, text: impl Into<String>) {
        let text = text.into();
        self.text_implicit = false;
        if self.number.is_none()
            && let Some(number) = default_number_for_text(&text)
        {
            self.number = Some(number);
            self.number_implicit = true;
        }
        self.text = Some(text);
    }

    /// Sets the note value the number counts, leaving the number alone — so
    /// the tempo itself changes.
    pub fn set_referent(&mut self, referent: Duration) {
        self.referent = referent;
    }

    /// The tempo word, if any.
    pub fn text(&self) -> Option<&str> {
        self.text.as_deref()
    }

    /// The tempo word as something a score would carry: music21's
    /// `getTextExpression`. A word only implied from the number is not
    /// answered unless `return_implicit` asks for it.
    pub fn text_expression(&self, return_implicit: bool) -> Option<&str> {
        if self.text_implicit && !return_implicit {
            return None;
        }
        self.text.as_deref()
    }

    /// The note value the number counts.
    pub fn referent(&self) -> &Duration {
        &self.referent
    }

    /// Whether the number was implied from the tempo word.
    pub fn number_implicit(&self) -> bool {
        self.number_implicit
    }

    /// Says so, or unsays it. music21 lets a caller write this: its MIDI
    /// reader copies a mark onto every staff and marks the copies implicit,
    /// which is how the parts after the first hide the number.
    pub fn set_number_implicit(&mut self, implicit: bool) {
        self.number_implicit = implicit;
    }

    /// Whether the tempo word was implied from the number.
    pub fn text_implicit(&self) -> bool {
        self.text_implicit
    }

    /// music21's `numberSounding`: how fast the mark is actually played,
    /// where that is not the number written over the staff.
    ///
    /// A score may say *Allegro* and be played at a hundred and forty-four,
    /// and a MusicXML `<sound tempo=...>` with no metronome mark beside it
    /// is a tempo that sounds and is never written. Nothing here is
    /// answered from it unless it is asked for: [`Self::quarter_bpm`] reads
    /// the written number, and [`Self::sounding_quarter_bpm`] reads this.
    pub fn number_sounding(&self) -> Option<FloatType> {
        self.number_sounding
    }

    /// Sets how fast the mark is played, apart from what it says.
    pub fn set_number_sounding(&mut self, number: Option<FloatType>) {
        self.number_sounding = number;
    }

    /// A mark built to be played at this speed.
    ///
    /// It does not become the mark's number: a mark that says nothing and
    /// sounds at a hundred and sixty-eight still says nothing, which is how
    /// music21 carries a tempo that is played and never written.
    pub fn with_number_sounding(mut self, number: FloatType) -> Self {
        self.number_sounding = Some(number);
        self
    }

    /// The tempo as quarter notes per minute, whatever the referent.
    pub fn quarter_bpm(&self) -> Option<FloatType> {
        self.number
            .map(|number| convert_tempo_by_referent(number, self.referent.quarter_length(), 1.0))
    }

    /// The same, at the speed the mark is played rather than the one it
    /// says: music21's `getQuarterBPM(useNumberSounding=True)`.
    pub fn sounding_quarter_bpm(&self) -> Option<FloatType> {
        match self.number_sounding {
            Some(number) => Some(convert_tempo_by_referent(
                number,
                self.referent.quarter_length(),
                1.0,
            )),
            None => self.quarter_bpm(),
        }
    }

    /// Seconds each quarter note lasts.
    ///
    /// A mark of nought beats a minute is refused rather than answered with
    /// an infinity, which is what dividing by it gives and what every
    /// arithmetic downstream of it would then carry.
    pub fn seconds_per_quarter(&self) -> Result<FloatType> {
        let quarter_bpm = self.quarter_bpm().ok_or_else(|| {
            Error::Tempo("cannot derive seconds without a tempo number".to_string())
        })?;
        if quarter_bpm == 0.0 {
            return Err(Error::Tempo(
                "a tempo of no beats a minute lasts no seconds a beat".to_string(),
            ));
        }
        Ok(60.0 / quarter_bpm)
    }

    /// Seconds a span of the given quarter length lasts at this tempo.
    pub fn quarter_length_to_seconds(&self, quarter_length: FloatType) -> Result<FloatType> {
        Ok(self.seconds_per_quarter()? * quarter_length)
    }

    /// Seconds a duration lasts at this tempo.
    pub fn duration_to_seconds(&self, duration: &Duration) -> Result<FloatType> {
        self.quarter_length_to_seconds(duration.quarter_length())
    }

    /// The duration that lasts the given number of seconds at this tempo.
    pub fn seconds_to_duration(&self, seconds: FloatType) -> Result<Duration> {
        if seconds.is_nan() || seconds <= 0.0 {
            return Err(Error::Tempo(
                "seconds must be a number greater than zero".to_string(),
            ));
        }
        Duration::new(seconds / self.seconds_per_quarter()?)
    }
}

/// Carries what lies between two points of one stream into another, keeping
/// each thing's place between them: music21's `interpolateElements`.
///
/// `start` and `end` each say where one point stands in the source and where
/// it stands in the destination -- a downbeat at offset 10 of a score and
/// 20.5 of a recording, the next at 14 and at 25. Everything strictly
/// between them in the source is inserted into the destination at the same
/// proportion of the way across, so a note at 11 lands at 21.625.
///
/// music21 finds the two points by asking one object for its offset in each
/// stream. A stream here owns what it holds, so there is no such object to
/// ask and the offsets are given instead; and for the same reason this is
/// always music21's `autoAdd`, since nothing can already be in both.
pub fn interpolate_elements(
    source: &Stream,
    destination: &mut Stream,
    start: (FloatType, FloatType),
    end: (FloatType, FloatType),
) -> Result<()> {
    let (start_source, end_source) = (start.0, end.0);
    // Checked before the walk, so two points at one offset are refused even
    // with nothing between them.
    interpolated_offset(start_source, start, end)?;
    for event in source.events() {
        if event.offset() > start_source && event.offset() < end_source {
            destination.insert(
                interpolated_offset(event.offset(), start, end)?,
                event.element().clone(),
            );
        }
    }
    Ok(())
}

/// Where a thing standing at `offset` in the source lands in the destination,
/// the same proportion of the way between the two points: the arithmetic of
/// [`interpolate_elements`], for a caller that moves things of its own.
///
/// `start` and `end` are each a point's offset in the source and in the
/// destination. It is an error for the two to stand at one offset in the
/// source, since then nothing lies between them to scale.
///
/// ```
/// use music21_rs::tempo::interpolated_offset;
///
/// assert_eq!(interpolated_offset(11.0, (10.0, 20.5), (14.0, 25.0))?, 21.625);
/// # Ok::<(), music21_rs::Error>(())
/// ```
pub fn interpolated_offset(
    offset: FloatType,
    start: (FloatType, FloatType),
    end: (FloatType, FloatType),
) -> Result<FloatType> {
    let ((start_source, start_destination), (end_source, end_destination)) = (start, end);
    if end_source == start_source {
        return Err(Error::Tempo(
            "the two points stand at the same offset in the source, so nothing lies between them"
                .to_string(),
        ));
    }
    let scale = (end_destination - start_destination) / (end_source - start_source);
    Ok(scale * (offset - start_source) + start_destination)
}

/// Which side of a [`MetricModulation`] something is on: the mark in force
/// before it, or the mark after. music21 calls them `'left'` and `'right'`.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ModulationSide {
    /// The mark in force before the modulation: music21's `'left'`.
    Old,
    /// The mark in force after it: music21's `'right'`.
    New,
}

/// A tempo said in words and not in numbers: music21's `TempoText`.
///
/// ```
/// use music21_rs::tempo::TempoText;
///
/// let slow = TempoText::new("slow");
/// assert_eq!(slow.metronome_mark().number(), Some(56.0));
/// assert!(TempoText::new("Largo e piano").is_common_tempo_text());
/// assert!(!TempoText::new("undulating").is_common_tempo_text());
/// ```
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[must_use]
pub struct TempoText {
    text: String,
}

impl TempoText {
    /// A tempo said as `text`.
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }

    /// The words the tempo is said in.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Says the tempo in other words.
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
    }

    /// The metronome mark the words imply: music21's `getMetronomeMark`.
    pub fn metronome_mark(&self) -> MetronomeMark {
        MetronomeMark::from_text(self.text.clone())
    }

    /// Whether the words read as a tempo: music21's `isCommonTempoText`,
    /// which looks for one of its tempo words inside them, or them inside
    /// one, ignoring case, spaces and full stops.
    pub fn is_common_tempo_text(&self) -> bool {
        is_common_tempo_text(&self.text)
    }
}

/// Whether `text` reads as a tempo, as [`TempoText::is_common_tempo_text`]
/// asks it.
pub fn is_common_tempo_text(text: &str) -> bool {
    let stripped = |value: &str| {
        value
            .trim()
            .chars()
            .filter(|character| *character != ' ' && *character != '.')
            .collect::<String>()
            .to_lowercase()
    };
    let text = stripped(text);
    DEFAULT_TEMPO_VALUES.iter().any(|(candidate, _)| {
        let candidate = stripped(candidate);
        text.contains(&candidate) || candidate.contains(&text)
    })
}

/// A change of tempo written as an equation between two metronome marks:
/// music21's `MetricModulation`.
///
/// The usual one keeps a number and moves it to another note value --
/// quarter = 60 becoming dotted quarter = 60 -- so the old mark and the new
/// are what the equation is written with. Either side may be given by its
/// note value alone and take its number from the other, and the old side may
/// take it from the mark in force before the modulation, which a score knows
/// and this type is handed ([`MetricModulation::update_from`]).
///
/// ```
/// use music21_rs::{tempo::{MetricModulation, MetronomeMark}, Duration};
///
/// let mut modulation = MetricModulation::new();
/// modulation.set_old_metronome(Some(MetronomeMark::new(60.0)));
/// modulation.set_new_referent(Duration::half());
/// assert_eq!(modulation.number(), Some(60.0));
/// assert_eq!(modulation.new_metronome().unwrap().quarter_bpm(), Some(120.0));
/// ```
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[must_use]
pub struct MetricModulation {
    old: Option<MetronomeMark>,
    new: Option<MetronomeMark>,
    classical_style: bool,
    maintain_beat: bool,
    transition_symbol: String,
    arrow_direction: Option<String>,
    parentheses: bool,
}

impl Default for MetricModulation {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricModulation {
    /// A modulation between two marks not yet given, written with `=`.
    pub fn new() -> Self {
        Self {
            old: None,
            new: None,
            classical_style: false,
            maintain_beat: false,
            transition_symbol: "=".to_string(),
            arrow_direction: None,
            parentheses: false,
        }
    }

    /// The mark in force before the modulation, if one is given.
    pub fn old_metronome(&self) -> Option<&MetronomeMark> {
        self.old.as_ref()
    }

    /// The mark in force after it, if one is given.
    pub fn new_metronome(&self) -> Option<&MetronomeMark> {
        self.new.as_ref()
    }

    /// Sets the mark in force before the modulation, or takes it away.
    pub fn set_old_metronome(&mut self, mark: Option<MetronomeMark>) {
        self.old = mark;
    }

    /// Sets the mark in force after the modulation, or takes it away.
    pub fn set_new_metronome(&mut self, mark: Option<MetronomeMark>) {
        self.new = mark;
    }

    /// The note value the old mark counts.
    pub fn old_referent(&self) -> Option<&Duration> {
        self.old.as_ref().map(MetronomeMark::referent)
    }

    /// The note value the new mark counts.
    pub fn new_referent(&self) -> Option<&Duration> {
        self.new.as_ref().map(MetronomeMark::referent)
    }

    /// Counts the old side in `referent`: the old mark's own tempo counted
    /// that way, or where there is none the tempo of `previous` -- the mark
    /// in force before the modulation -- counted that way, or failing both a
    /// mark with no number yet. music21's `oldReferent` setter.
    pub fn set_old_referent(&mut self, referent: Duration, previous: Option<&MetronomeMark>) {
        self.old = Some(match (&self.old, previous) {
            (Some(old), _) => old.equivalent_by_referent(referent),
            (None, Some(previous)) => previous.equivalent_by_referent(referent),
            (None, None) => MetronomeMark::default().with_referent(referent),
        });
    }

    /// Counts the new side in `referent`: the new mark's own tempo counted
    /// that way, or where there is none the old mark's *number* moved to
    /// that note value -- the modulation proper -- or failing both a mark
    /// with no number yet. music21's `newReferent` setter.
    pub fn set_new_referent(&mut self, referent: Duration) {
        self.new = Some(match (&self.new, &self.old) {
            (Some(new), _) => new.equivalent_by_referent(referent),
            (None, Some(old)) => old.maintained_number_with_referent(referent),
            (None, None) => MetronomeMark::default().with_referent(referent),
        });
    }

    /// The number of the new mark, which is what the modulation sets.
    pub fn number(&self) -> Option<FloatType> {
        self.new.as_ref().and_then(MetronomeMark::number)
    }

    /// Fills in what the marks leave unsaid from `previous`, the mark in
    /// force before the modulation: music21's `updateByContext`, with the
    /// context search done by whoever knows the score.
    ///
    /// The old side becomes `previous` counted in the old side's note value,
    /// or `previous` itself where there is no old side; and a new side with
    /// a note value takes the old side's number, since the number is what a
    /// modulation keeps.
    pub fn update_from(&mut self, previous: Option<&MetronomeMark>) {
        if let Some(previous) = previous {
            self.old = Some(match &self.old {
                Some(old) => previous.equivalent_by_referent(old.referent().clone()),
                None => previous.maintained_number_with_referent(previous.referent().clone()),
            });
        }
        let number = self.old.as_ref().and_then(MetronomeMark::number);
        if let (Some(new), Some(number)) = (self.new.as_mut(), number) {
            new.set_number(Some(number));
        }
    }

    /// Sets one side to the same tempo as the other counted in `referent`:
    /// music21's `setEqualityByReferent`. With no side named, the side not
    /// yet given is the one set; it is an error when both are given or when
    /// the other side is missing.
    pub fn set_equality_by_referent(
        &mut self,
        side: Option<ModulationSide>,
        referent: Duration,
    ) -> Result<()> {
        match self.side_to_set(side)? {
            ModulationSide::New => {
                let old = self.old.as_ref().ok_or_else(Self::no_other_side)?;
                self.new = Some(old.equivalent_by_referent(referent));
            }
            ModulationSide::Old => {
                let new = self.new.as_ref().ok_or_else(Self::no_other_side)?;
                self.old = Some(new.equivalent_by_referent(referent));
            }
        }
        Ok(())
    }

    /// Sets one side to the other's number counted in `referent`, which is a
    /// different tempo: music21's `setOtherByReferent`. The side is chosen
    /// as [`MetricModulation::set_equality_by_referent`] chooses it.
    pub fn set_other_by_referent(
        &mut self,
        side: Option<ModulationSide>,
        referent: Duration,
    ) -> Result<()> {
        match self.side_to_set(side)? {
            ModulationSide::New => {
                let old = self.old.as_ref().ok_or_else(Self::no_other_side)?;
                self.new = Some(old.maintained_number_with_referent(referent));
            }
            ModulationSide::Old => {
                let new = self.new.as_ref().ok_or_else(Self::no_other_side)?;
                self.old = Some(new.maintained_number_with_referent(referent));
            }
        }
        Ok(())
    }

    fn side_to_set(&self, side: Option<ModulationSide>) -> Result<ModulationSide> {
        side.or(match (&self.old, &self.new) {
            (None, _) => Some(ModulationSide::Old),
            (_, None) => Some(ModulationSide::New),
            _ => None,
        })
        .ok_or_else(|| Error::Tempo("cannot set equality for a side of None".to_string()))
    }

    fn no_other_side() -> Error {
        Error::Tempo("there is no mark on the other side to take the tempo from".to_string())
    }

    /// Whether the first mark written is the new tempo rather than the old,
    /// the reverse of the usual order: music21's `classicalStyle`.
    pub fn classical_style(&self) -> bool {
        self.classical_style
    }

    /// Sets whether the first mark written is the new tempo.
    pub fn set_classical_style(&mut self, classical: bool) {
        self.classical_style = classical;
    }

    /// Whether the beat is kept after the equation, as it is going from 3/4
    /// to 6/8: music21's `maintainBeat`.
    pub fn maintain_beat(&self) -> bool {
        self.maintain_beat
    }

    /// Sets whether the beat is kept after the equation.
    pub fn set_maintain_beat(&mut self, maintain: bool) {
        self.maintain_beat = maintain;
    }

    /// What the equation is written with, `=` unless an edition says
    /// otherwise.
    pub fn transition_symbol(&self) -> &str {
        &self.transition_symbol
    }

    /// Sets what the equation is written with.
    pub fn set_transition_symbol(&mut self, symbol: impl Into<String>) {
        self.transition_symbol = symbol.into();
    }

    /// Which way an arrow drawn for the equation points, where older
    /// editions draw one: `"left"`, `"right"`, or none.
    pub fn arrow_direction(&self) -> Option<&str> {
        self.arrow_direction.as_deref()
    }

    /// Sets which way the arrow points, or takes it away.
    pub fn set_arrow_direction(&mut self, direction: Option<String>) {
        self.arrow_direction = direction;
    }

    /// Whether the equation is written in parentheses.
    pub fn parentheses(&self) -> bool {
        self.parentheses
    }

    /// Sets whether the equation is written in parentheses.
    pub fn set_parentheses(&mut self, parentheses: bool) {
        self.parentheses = parentheses;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_mark_can_be_rewritten_one_half_at_a_time() {
        let mut mark = MetronomeMark::from_text("adagio");
        assert!(mark.number_implicit());
        mark.set_number(Some(60.0));
        assert!(!mark.number_implicit());
        assert_eq!(mark.number(), Some(60.0));
        mark.set_number_implicit(true);
        assert!(mark.number_implicit());
        mark.set_referent(Duration::half());
        assert_eq!(mark.referent().quarter_length(), 2.0);
        assert_eq!(mark.quarter_bpm(), Some(120.0));

        let mut numbered = MetronomeMark::new(90.0);
        assert!(numbered.text_implicit());
        numbered.set_text("largo");
        assert!(!numbered.text_implicit());
        assert_eq!(numbered.text(), Some("largo"));
        assert_eq!(numbered.number(), Some(90.0));
        let mut wordless = MetronomeMark::default();
        wordless.set_number(Some(184.0));
        assert_eq!(wordless.text(), Some("presto"));
        assert!(wordless.text_implicit());
        wordless.set_number(None);
        assert_eq!(wordless.number(), None);
    }

    /// music21's own `getTextExpression` examples.
    #[test]
    fn an_implied_word_is_only_answered_when_asked_for() {
        let presto = MetronomeMark::from_text("presto");
        assert_eq!(presto.number(), Some(184.0));
        assert_eq!(presto.text_expression(false), Some("presto"));

        let ninety = MetronomeMark::new(90.0);
        assert!(ninety.text_implicit());
        assert_eq!(ninety.text_expression(false), None);
        assert_eq!(ninety.text_expression(true), Some("maestoso"));
    }

    #[test]
    fn a_mark_may_be_played_faster_than_it_is_written() {
        // music21's own example: a tempo that sounds and is never written.
        let playback = MetronomeMark::default().with_number_sounding(168.0);
        assert_eq!(playback.number_sounding(), Some(168.0));
        // It says nothing: the tempo is played and never written.
        assert_eq!(playback.number(), None);
        assert_eq!(playback.text(), None);
        assert_eq!(playback.quarter_bpm(), None);
        assert_eq!(playback.sounding_quarter_bpm(), Some(168.0));

        // A mark that says a number keeps it, and only the sounding tempo
        // reads the other.
        let mut written = MetronomeMark::new(60.0).with_referent(Duration::half());
        assert_eq!(written.quarter_bpm(), Some(120.0));
        assert_eq!(written.sounding_quarter_bpm(), Some(120.0));
        written.set_number_sounding(Some(90.0));
        assert_eq!(written.number(), Some(60.0));
        assert_eq!(written.quarter_bpm(), Some(120.0));
        assert_eq!(written.sounding_quarter_bpm(), Some(180.0));
    }

    #[test]
    fn referent_changes_match_music21() {
        let quarter_sixty = MetronomeMark::new(60.0);
        let eighths = quarter_sixty.equivalent_by_referent(Duration::eighth());
        assert_eq!(eighths.number(), Some(120.0));
        assert_eq!(eighths.referent().quarter_length(), 0.5);
        assert_eq!(eighths.text(), Some("larghetto"));
        let halves = quarter_sixty.equivalent_by_referent(Duration::half());
        assert_eq!(halves.number(), Some(30.0));

        let andante = MetronomeMark::with_number_and_text(72.0, "andante")
            .with_referent(Duration::new(1.5).unwrap());
        let quarters = andante.equivalent_by_referent(Duration::quarter());
        assert_eq!(quarters.number(), Some(108.0));
        assert_eq!(quarters.text(), Some("andante"));
        assert_eq!(
            andante.equivalent_by_referent(Duration::half()).number(),
            Some(54.0)
        );

        let kept = andante.maintained_number_with_referent(Duration::eighth());
        assert_eq!(kept.number(), Some(72.0));
        assert_eq!(kept.referent().quarter_length(), 0.5);
        assert_eq!(kept.text(), Some("andante"));
    }

    #[test]
    fn numbers_imply_the_tempo_words_music21_picks() {
        let cases = [
            (120.0, Some("animato")),
            (60.0, Some("larghetto")),
            (61.0, Some("larghetto")),
            (62.0, Some("larghetto")),
            (63.0, None),
            (90.0, Some("maestoso")),
            (92.0, Some("moderate")),
            (100.0, None),
            (118.0, Some("animato")),
            (122.0, Some("animato")),
            (123.0, None),
            (16.0, Some("larghissimo")),
            (12.0, None),
            (56.5, Some("adagio")),
            (300.0, None),
        ];
        for (number, text) in cases {
            let mark = MetronomeMark::new(number);
            assert_eq!(mark.text(), text, "{number}");
            assert_eq!(mark.text_implicit(), text.is_some(), "{number}");
            assert!(!mark.number_implicit());
        }
    }

    #[test]
    fn words_imply_their_conventional_numbers() {
        let cases = [
            ("allegro", Some(132.0)),
            ("Allegro", Some(132.0)),
            ("ALLEGRO ", Some(132.0)),
            ("très vite", Some(144.0)),
            ("andante moderato", Some(83.0)),
            ("unknown words", None),
        ];
        for (text, number) in cases {
            let mark = MetronomeMark::from_text(text);
            assert_eq!(mark.number(), number, "{text}");
            assert_eq!(mark.number_implicit(), number.is_some(), "{text}");
            assert_eq!(mark.text(), Some(text));
            assert!(!mark.text_implicit());
        }
        let both = MetronomeMark::with_number_and_text(120.0, "fast");
        assert_eq!(both.number(), Some(120.0));
        assert_eq!(both.text(), Some("fast"));
        assert!(!both.number_implicit() && !both.text_implicit());
    }

    #[test]
    fn referents_convert_to_quarter_bpm_and_seconds() {
        let quarter = MetronomeMark::new(120.0);
        assert_eq!(quarter.quarter_bpm(), Some(120.0));
        assert_eq!(quarter.quarter_length_to_seconds(1.0).unwrap(), 0.5);
        assert_eq!(
            quarter.seconds_to_duration(0.75).unwrap().quarter_length(),
            1.5
        );

        let half = MetronomeMark::new(60.0).with_referent(Duration::half());
        assert_eq!(half.quarter_bpm(), Some(120.0));
        assert_eq!(half.text(), Some("larghetto"));
        assert_eq!(half.quarter_length_to_seconds(1.0).unwrap(), 0.5);
        assert_eq!(
            half.duration_to_seconds(&Duration::new(3.0).unwrap())
                .unwrap(),
            1.5
        );
        assert_eq!(half.seconds_to_duration(1.0).unwrap().quarter_length(), 2.0);

        let eighth = MetronomeMark::new(120.0).with_referent(Duration::eighth());
        assert_eq!(eighth.quarter_bpm(), Some(60.0));
        assert_eq!(eighth.seconds_per_quarter().unwrap(), 1.0);

        let dotted = MetronomeMark::new(56.5).with_referent(Duration::half());
        assert!((dotted.quarter_bpm().unwrap() - 113.0).abs() < 1e-9);

        assert_eq!(convert_tempo_by_referent(60.0, 1.0, 2.0), 30.0);
        assert_eq!(convert_tempo_by_referent(60.0, 2.0, 1.0), 120.0);

        let unknown = MetronomeMark::from_text("unknown words");
        assert_eq!(unknown.quarter_bpm(), None);
        assert!(unknown.seconds_per_quarter().is_err());
        assert!(quarter.seconds_to_duration(0.0).is_err());

        let motionless = MetronomeMark::new(0.0);
        assert_eq!(motionless.quarter_bpm(), Some(0.0));
        assert!(motionless.seconds_per_quarter().is_err());
        assert!(
            motionless
                .duration_to_seconds(&Duration::quarter())
                .is_err()
        );
        assert!(motionless.quarter_length_to_seconds(1.0).is_err());
    }

    /// music21's own example: a bar of a score laid against a recording.
    #[test]
    fn what_lies_between_two_points_keeps_its_place_between_them() {
        use crate::{Note, Stream};

        let mut source = Stream::new();
        for (offset, name) in [
            (10.0, "C4"),
            (11.0, "D4"),
            (12.0, "E4"),
            (13.0, "F4"),
            (14.0, "G4"),
        ] {
            source.insert(offset, Note::from_name(name).unwrap());
        }
        let offsets = |stream: &Stream| -> Vec<FloatType> {
            stream.events().iter().map(|event| event.offset()).collect()
        };

        let mut recording = Stream::new();
        interpolate_elements(&source, &mut recording, (10.0, 20.5), (14.0, 25.0)).unwrap();
        assert_eq!(offsets(&recording), [21.625, 22.75, 23.875]);

        let mut slower = Stream::new();
        interpolate_elements(&source, &mut slower, (10.0, 10.1), (14.0, 50.5)).unwrap();
        for (found, expected) in offsets(&slower).iter().zip([20.2, 30.3, 40.4]) {
            assert!((found - expected).abs() < 1e-9);
        }

        assert!(interpolate_elements(&source, &mut slower, (10.0, 0.0), (10.0, 4.0)).is_err());
    }

    #[test]
    fn a_metric_modulation_moves_a_number_to_another_note_value() {
        // Each answer read off music21 11.0.0b9's MetricModulation.
        use super::{MetricModulation, MetronomeMark, ModulationSide};
        use crate::Duration;

        // Setting both referents first leaves both numbers unsaid.
        let mut modulation = MetricModulation::new();
        modulation.set_old_referent(Duration::half(), None);
        modulation.set_new_referent(Duration::new(0.25).unwrap());
        assert_eq!(modulation.number(), None);

        // Then an old mark with a number: the new side takes the number.
        let mut modulation = MetricModulation::new();
        modulation.set_old_referent(Duration::quarter(), None);
        modulation.set_new_referent(Duration::eighth());
        modulation.set_old_metronome(Some(MetronomeMark::new(60.0)));
        modulation.update_from(None);
        assert_eq!(modulation.number(), Some(60.0));
        assert_eq!(
            modulation.new_metronome().unwrap().text(),
            Some("larghetto")
        );

        // Equality keeps the tempo and recounts it: half = 30 is quarter = 60.
        let mut modulation = MetricModulation::new();
        modulation.set_new_metronome(Some(MetronomeMark::new(60.0)));
        modulation
            .set_equality_by_referent(None, Duration::half())
            .unwrap();
        assert_eq!(modulation.old_metronome().unwrap().number(), Some(30.0));

        // The other translation keeps the number and changes the tempo.
        let mut modulation = MetricModulation::new();
        modulation.set_old_metronome(Some(MetronomeMark::new(60.0)));
        modulation
            .set_other_by_referent(Some(ModulationSide::New), Duration::half())
            .unwrap();
        assert_eq!(modulation.number(), Some(60.0));
        assert_eq!(
            modulation.new_metronome().unwrap().quarter_bpm(),
            Some(120.0)
        );

        // With both sides given there is no side to choose.
        assert!(
            modulation
                .set_equality_by_referent(None, Duration::quarter())
                .is_err()
        );

        // The mark in force before fills in an old side counted in eighths.
        let mut modulation = MetricModulation::new();
        modulation.set_old_referent(Duration::eighth(), None);
        modulation.update_from(Some(&MetronomeMark::new(90.0)));
        assert_eq!(modulation.old_metronome().unwrap().number(), Some(180.0));
    }
}
