//! Metronome marks, a port of the stream-free part of music21's `tempo`.

use crate::{
    defaults::FloatType,
    duration::Duration,
    error::{Error, Result},
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
pub struct MetronomeMark {
    number: Option<FloatType>,
    text: Option<String>,
    referent: Duration,
    number_implicit: bool,
    text_implicit: bool,
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
        }
    }

    /// The beats per minute, if known.
    pub fn number(&self) -> Option<FloatType> {
        self.number
    }

    /// The tempo word, if any.
    pub fn text(&self) -> Option<&str> {
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

    /// Whether the tempo word was implied from the number.
    pub fn text_implicit(&self) -> bool {
        self.text_implicit
    }

    /// The tempo as quarter notes per minute, whatever the referent.
    pub fn quarter_bpm(&self) -> Option<FloatType> {
        self.number
            .map(|number| convert_tempo_by_referent(number, self.referent.quarter_length(), 1.0))
    }

    /// Seconds each quarter note lasts.
    pub fn seconds_per_quarter(&self) -> Result<FloatType> {
        self.quarter_bpm()
            .map(|bpm| 60.0 / bpm)
            .ok_or_else(|| Error::Tempo("cannot derive seconds without a tempo number".to_string()))
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

#[cfg(test)]
mod tests {
    use super::*;

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
    }
}
