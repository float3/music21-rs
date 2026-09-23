use crate::defaults::IntegerType;
use crate::duration::Duration;
use crate::notation::{Lyric, verses};

/// A silent musical event with a duration.
///
/// A rest may still carry words: a syllable written under a rest is how a
/// score says a line is spoken there, or how a verse number sits at the start
/// of a phrase that opens on a rest. music21 writes the lyric methods once on
/// `GeneralNote`, and a rest has them as a note does.
///
/// Two rests are equal when they last as long; what is written under them
/// does not count, as it does not in music21.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[must_use]
pub struct Rest {
    duration: Duration,
    #[cfg_attr(feature = "serde", serde(default))]
    lyrics: Vec<Lyric>,
}

impl PartialEq for Rest {
    fn eq(&self, other: &Self) -> bool {
        self.duration == other.duration
    }
}

impl Rest {
    /// Creates a rest with the supplied duration.
    pub fn new(duration: Duration) -> Self {
        Self {
            duration,
            lyrics: Vec::new(),
        }
    }

    /// Creates a rest from a quarter-length value.
    pub fn from_quarter_length(quarter_length: crate::FloatType) -> crate::Result<Self> {
        Ok(Self::new(Duration::new(quarter_length)?))
    }

    /// Returns the rest duration.
    pub fn duration(&self) -> &Duration {
        &self.duration
    }

    /// Updates the rest duration.
    pub fn set_duration(&mut self, duration: Duration) {
        self.duration = duration;
    }

    /// Returns the duration in quarter lengths.
    pub fn quarter_length(&self) -> crate::FloatType {
        self.duration.quarter_length()
    }

    /// The most complete name of the rest, its length included: music21's
    /// `fullName`, `Dotted Quarter Rest`.
    pub fn full_name(&self) -> String {
        format!("{} Rest", self.duration.full_name())
    }

    /// The syllables written under the rest, one per verse.
    pub fn lyrics(&self) -> &[Lyric] {
        &self.lyrics
    }

    /// The syllables written under the rest, for editing in place.
    pub fn lyrics_mut(&mut self) -> &mut Vec<Lyric> {
        &mut self.lyrics
    }

    /// The text of every verse, one per line: music21's `lyric`.
    pub fn lyric(&self) -> Option<String> {
        verses::joined(&self.lyrics)
    }

    /// Replaces every verse with one per line of the text, or clears them
    /// with `None`: music21's `lyric` setter.
    pub fn set_lyric(&mut self, lyric: Option<&str>) {
        verses::set(&mut self.lyrics, lyric);
    }

    /// Adds a syllable as a verse: music21's `addLyric`, which a rest has
    /// as a note has it. See [`crate::Note::add_lyric`].
    pub fn add_lyric(&mut self, text: &str, number: Option<IntegerType>, apply_raw: bool) {
        verses::add(&mut self.lyrics, text, number, apply_raw);
    }

    /// Puts a syllable in front of the verse at `index`: music21's
    /// `insertLyric`.
    pub fn insert_lyric(&mut self, text: &str, index: usize, apply_raw: bool) {
        verses::insert(&mut self.lyrics, text, index, apply_raw);
    }
}

impl Default for Rest {
    fn default() -> Self {
        Self::new(Duration::default())
    }
}

impl From<Duration> for Rest {
    fn from(value: Duration) -> Self {
        Self::new(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rest_has_duration() {
        let rest = Rest::from_quarter_length(2.0).unwrap();
        assert_eq!(rest.quarter_length(), 2.0);
    }

    #[test]
    fn a_rest_is_named_by_its_length() {
        assert_eq!(
            Rest::from_quarter_length(1.5).unwrap().full_name(),
            "Dotted Quarter Rest"
        );
        assert_eq!(Rest::new(Duration::whole()).full_name(), "Whole Rest");
    }

    #[test]
    fn a_rest_carries_verses_as_a_note_does_and_they_do_not_make_it_unequal() {
        let mut rest = Rest::from_quarter_length(1.0).unwrap();
        rest.add_lyric("hel-", None, false);
        rest.add_lyric("two", None, false);
        rest.insert_lyric("one", 0, false);
        assert_eq!(rest.lyric().as_deref(), Some("one\nhel\ntwo"));
        assert_eq!(
            rest.lyrics().iter().map(Lyric::number).collect::<Vec<_>>(),
            [1, 2, 3]
        );
        assert_eq!(rest, Rest::from_quarter_length(1.0).unwrap());
        rest.set_lyric(None);
        assert!(rest.lyrics().is_empty());
    }

    #[test]
    fn rest_supports_default_from_and_updates() {
        let mut rest = Rest::default();
        assert_eq!(rest.quarter_length(), 1.0);

        rest.set_duration(Duration::whole());
        assert_eq!(rest.duration(), &Duration::whole());

        let half_rest = Rest::from(Duration::half());
        assert_eq!(half_rest.quarter_length(), 2.0);
        assert!(Rest::from_quarter_length(crate::FloatType::NAN).is_err());
    }
}
