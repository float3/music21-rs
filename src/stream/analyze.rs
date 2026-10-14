//! Asking a stream for an analysis by name, and the two questions it
//! answers about itself: music21's `Stream.analyze`, `isTwelveTone` and
//! `notesAndRests`.

use super::{Stream, StreamElement, StreamEvent};
use crate::analysis::{KeyProfile, estimate_key_of_stream, stream_pitch_span};
use crate::defaults::FloatType;
use crate::error::{Error, Result};
use crate::interval::Interval;
use crate::key::Key;

/// What an analysis of a stream answers, which in music21 depends on the
/// method asked for: a key, or the span of its pitches.
#[derive(Clone, Debug)]
pub enum Analysis {
    /// The key the stream is most likely in, as the key-weighting
    /// analyses name it.
    Key(Key),
    /// The interval from the lowest pitch sounding to the highest:
    /// music21's `Ambitus`.
    Ambitus(Interval),
}

/// Which analysis a method name asks for: music21's
/// `analysisClassFromMethodName`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Method {
    /// The span of the pitches.
    Ambitus,
    /// One of the key weightings.
    Key(KeyProfile),
}

impl Method {
    /// The class name music21 gives it, which a method name is matched
    /// against first.
    fn class_name(self) -> &'static str {
        match self {
            Self::Ambitus => "Ambitus",
            Self::Key(profile) => profile.music21_class_name(),
        }
    }

    /// The name music21 calls it by, matched against next.
    fn spoken_name(self) -> &'static str {
        match self {
            Self::Ambitus => "Ambitus Analysis",
            Self::Key(KeyProfile::KrumhanslSchmuckler) => {
                "Krumhansl Schmuckler/Kessler Key Analysis"
            }
            Self::Key(KeyProfile::AardenEssen) => "Aarden Essen Key Analysis",
            Self::Key(KeyProfile::SimpleWeights) => "Simple Weight Key Analysis",
            Self::Key(KeyProfile::BellmanBudge) => "Bellman Budge Key Analysis",
            Self::Key(KeyProfile::TemperleyKostkaPayne) => "Temperley Kostka Payne Key Analysis",
        }
    }

    /// The names it answers to: music21's `identifiers`.
    fn identifiers(self) -> &'static [&'static str] {
        match self {
            Self::Ambitus => &["ambitus", "span"],
            Self::Key(KeyProfile::KrumhanslSchmuckler) => &[
                "key.krumhansl",
                "key.schmuckler",
                "key.krumhansl-schmuckler",
                "key.krumhanslschmuckler",
                "krumhansl",
                "schmuckler",
                "krumhansl-schmuckler",
                "krumhanslschmuckler",
                "key.kessler",
                "key.krumhansl-kessler",
                "key.krumhanslkessler",
                "kessler",
                "krumhansl-kessler",
                "krumhanslkessler",
            ],
            Self::Key(KeyProfile::AardenEssen) => &[
                "key.aarden",
                "key.essen",
                "key.aarden-essen",
                "key.aardenessen",
                "aarden",
                "essen",
                "aarden-essen",
                "aardenessen",
                // These make this the analysis a bare `key` asks for.
                "key",
                "keyscape",
            ],
            Self::Key(KeyProfile::SimpleWeights) => &[
                "key.simple",
                "key.weight",
                "key.simple-weight",
                "key.simpleweight",
                "simple",
                "weight",
                "simple-weight",
                "simpleweight",
            ],
            Self::Key(KeyProfile::BellmanBudge) => &[
                "key.bellman",
                "key.budge",
                "key.bellman-budge",
                "key.bellmanbudge",
                "bellman",
                "budge",
                "bellman-budge",
                "bellmanbudge",
            ],
            Self::Key(KeyProfile::TemperleyKostkaPayne) => &[
                "key.temperley",
                "key.kostka",
                "key.payne",
                "key.temperley-kostka-payne",
                "key.temperleykostkapayne",
                "temperley",
                "kostka",
                "payne",
                "temperley-kostka-payne",
                "temperleykostkapayne",
            ],
        }
    }
}

/// Every analysis a method name may ask for, in the order music21 searches
/// them, which decides which one a loose name finds first.
const METHODS: [Method; 6] = [
    Method::Ambitus,
    Method::Key(KeyProfile::KrumhanslSchmuckler),
    Method::Key(KeyProfile::AardenEssen),
    Method::Key(KeyProfile::SimpleWeights),
    Method::Key(KeyProfile::BellmanBudge),
    Method::Key(KeyProfile::TemperleyKostkaPayne),
];

/// The analysis a method name asks for: music21's
/// `analysisClassFromMethodName`, which looks three times over, loosest
/// last.
fn method_named(method: &str) -> Option<Method> {
    let lower = method.to_lowercase();
    // Anything the class name or the spoken name holds.
    let found = METHODS.into_iter().find(|candidate| {
        candidate.class_name().to_lowercase().contains(&lower)
            || candidate.spoken_name().contains(&lower)
    });
    if found.is_some() {
        return found;
    }
    // Then a name it answers to outright.
    let found = METHODS
        .into_iter()
        .find(|candidate| candidate.identifiers().contains(&method));
    if found.is_some() {
        return found;
    }
    // Then any name holding it.
    METHODS.into_iter().find(|candidate| {
        candidate
            .identifiers()
            .iter()
            .any(|identifier| identifier.contains(method))
    })
}

impl Stream {
    /// The analysis a method name asks for: music21's `analyze`.
    ///
    /// `ambitus`, `span` and `range` give the interval from the lowest
    /// pitch sounding to the highest; `key` and the names of music21's
    /// five key weightings give the key the stream is most likely in. The
    /// name need not be exact, music21 searching its own names three times
    /// over and loosest last, so `kostka` finds Temperley's profile and a
    /// bare `key` finds Aarden's, which is music21's default.
    ///
    /// ```
    /// use music21_rs::stream::Analysis;
    /// use music21_rs::{Note, Stream};
    ///
    /// let mut line = Stream::new();
    /// for name in ["C4", "E4", "G4", "C5"] {
    ///     line.push(Note::from_name(name)?);
    /// }
    /// let Some(Analysis::Ambitus(span)) = line.analyze("ambitus")? else {
    ///     panic!("an ambitus")
    /// };
    /// assert_eq!(span.short_name(), "P8");
    ///
    /// let Some(Analysis::Key(key)) = line.analyze("key")? else {
    ///     panic!("a key")
    /// };
    /// assert_eq!(key.tonic_pitch_name_with_case(), "C");
    ///
    /// // A stream with no pitches has no ambitus.
    /// assert!(Stream::new().analyze("ambitus")?.is_none());
    /// # Ok::<(), music21_rs::Error>(())
    /// ```
    ///
    /// A stream with no pitches has no ambitus and answers nothing for
    /// one, as music21 answers `None`; a key analysis of one refuses,
    /// which is what music21 raises on.
    ///
    /// # Errors
    ///
    /// A method name no analysis answers to, as music21 raises on one, or
    /// a key no weighting could settle on.
    pub fn analyze(&self, method: &str) -> Result<Option<Analysis>> {
        // music21 kept `range` as another name for `span` when it took
        // getPitchRanges out.
        let asked = if method == "range" { "span" } else { method };
        let Some(found) = method_named(asked) else {
            return Err(Error::Analysis(format!(
                "no such analysis method: {method}"
            )));
        };
        match found {
            Method::Ambitus => {
                let Some((lowest, highest)) = stream_pitch_span(self) else {
                    // music21 answers None rather than refusing.
                    return Ok(None);
                };
                Ok(Some(Analysis::Ambitus(Interval::between_pitches(
                    &lowest, &highest,
                )?)))
            }
            Method::Key(profile) => {
                let estimates = estimate_key_of_stream(profile, self).ok_or_else(|| {
                    Error::Analysis("a stream with no pitches has no key".to_string())
                })?;
                let best = estimates.first().ok_or_else(|| {
                    Error::Analysis("no key was estimated for the stream".to_string())
                })?;
                Ok(Some(Analysis::Key(best.key().clone())))
            }
        }
    }

    /// Whether every pitch sounding is one of the twelve of equal
    /// temperament: music21's `isTwelveTone`.
    ///
    /// A stream with no pitches is, as music21 answers for one.
    ///
    /// ```
    /// use music21_rs::{Note, Stream};
    ///
    /// let mut line = Stream::new();
    /// line.push(Note::from_name("G#4")?);
    /// assert!(line.is_twelve_tone());
    /// line.push(Note::from_name("G~4")?);
    /// assert!(!line.is_twelve_tone());
    /// # Ok::<(), music21_rs::Error>(())
    /// ```
    pub fn is_twelve_tone(&self) -> bool {
        self.pitches()
            .iter()
            .all(crate::pitch::Pitch::is_twelve_tone)
    }

    /// Every note, rest and chord on the flattened timeline: music21's
    /// `flatten().notesAndRests`.
    ///
    /// [`Self::notes`] leaves the rests out, as music21's `notes` does.
    /// For what the stream holds itself rather than the whole nesting, ask
    /// [`Self::elements_of_class`] for `GeneralNote`, which is music21's
    /// own `notesAndRests`.
    ///
    /// ```
    /// use music21_rs::{Duration, KeySignature, Note, Rest, Stream};
    ///
    /// let mut line = Stream::new();
    /// line.insert(0.0, KeySignature::new(2));
    /// line.push(Note::from_name("C4")?);
    /// line.push(Rest::new(Duration::quarter()));
    /// // The key signature is left out and the rest kept.
    /// assert_eq!(line.notes_and_rests().len(), 2);
    /// assert_eq!(line.notes().len(), 1);
    /// # Ok::<(), music21_rs::Error>(())
    /// ```
    pub fn notes_and_rests(&self) -> Vec<(FloatType, StreamElement)> {
        self.flatten()
            .events()
            .iter()
            .filter(|event| is_general_note(event))
            .map(|event| (event.offset(), event.element().clone()))
            .collect()
    }
}

/// Whether an element is one of music21's `GeneralNote`s.
fn is_general_note(event: &StreamEvent) -> bool {
    event.element().is_instance("GeneralNote")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::key::KeySignature;
    use crate::note::Note;
    use crate::rest::Rest;

    fn triad() -> Stream {
        let mut line = Stream::new();
        for name in ["C4", "E4", "G4", "C5"] {
            line.push(Note::from_name(name).expect("a note name"));
        }
        line
    }

    /// Each answer here is music21's own for the same stream.
    #[test]
    fn a_method_name_asks_for_an_analysis() {
        let line = triad();
        for name in ["ambitus", "span", "range", "Ambitus"] {
            let Some(Analysis::Ambitus(span)) = line.analyze(name).expect("an ambitus") else {
                panic!("an ambitus for {name}")
            };
            assert_eq!(span.short_name(), "P8");
        }
        for name in ["key", "keyscape", "aarden", "key.aarden"] {
            let Some(Analysis::Key(key)) = line.analyze(name).expect("a key") else {
                panic!("a key for {name}")
            };
            assert_eq!(key.tonic_pitch_name_with_case(), "C");
        }
        assert!(line.analyze("unknown-format").is_err());
        // A stream with no pitches has no ambitus but refuses a key, as
        // music21 answers None for the one and raises for the other.
        let bare = Stream::new();
        assert!(bare.analyze("ambitus").expect("no ambitus").is_none());
        assert!(bare.analyze("key").is_err());
    }

    #[test]
    fn a_loose_name_finds_what_music21_finds() {
        // Every one of these is the profile music21's own search lands on.
        for (name, profile) in [
            ("kostka", "TemperleyKostkaPayne"),
            ("payne", "TemperleyKostkaPayne"),
            ("kessler", "KrumhanslSchmuckler"),
            ("schmuckler", "KrumhanslSchmuckler"),
            ("budge", "BellmanBudge"),
            ("weight", "SimpleWeights"),
            ("essen", "AardenEssen"),
        ] {
            let found = method_named(name).unwrap_or_else(|| panic!("{name} names an analysis"));
            assert_eq!(found.class_name(), profile, "{name}");
        }
    }

    #[test]
    fn every_pitch_of_a_twelve_tone_stream_is_one_of_the_twelve() {
        let mut line = Stream::new();
        assert!(line.is_twelve_tone());
        line.push(Note::from_name("G#4").expect("a note name"));
        assert!(line.is_twelve_tone());
        line.push(Note::from_name("G~4").expect("a note name"));
        assert!(!line.is_twelve_tone());
    }

    #[test]
    fn the_rests_come_with_the_notes_and_the_marks_do_not() {
        let mut line = Stream::new();
        line.insert(0.0, KeySignature::new(2));
        line.push(Note::from_name("C4").expect("a note name"));
        line.push(Rest::new(crate::duration::Duration::quarter()));
        assert_eq!(line.notes_and_rests().len(), 2);
        assert_eq!(line.notes().len(), 1);
    }
}
