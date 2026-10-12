//! The scores a multi-work file holds: music21's `stream.Opus`.
//!
//! An ABC or Essen collection is one file holding many tunes, read into one
//! stream of kind [`StreamKind::Opus`] whose elements are scores. These
//! methods look a score up in it and merge the scores that are really one
//! work's parts.

use super::{Stream, StreamKind};
use crate::error::{Error, Result};
use crate::makenotation::make_notation;
use crate::metadata::{Metadata, MetadataQuery};

impl Stream {
    /// The scores this opus holds: music21's `Opus.scores`.
    pub fn scores(&self) -> Vec<&Stream> {
        self.streams_of_kind(StreamKind::Score)
    }

    /// Each score's number, as its metadata gives it: music21's
    /// `Opus.getNumbers`. A score whose metadata says nothing has none,
    /// where music21 raises on metadata it does not have.
    pub fn score_numbers(&self) -> Vec<Option<String>> {
        self.scores()
            .into_iter()
            .map(|score| {
                score
                    .metadata()
                    .and_then(|metadata| metadata.string_value("number"))
            })
            .collect()
    }

    /// The first score whose number a query matches: music21's
    /// `Opus.getScoreByNumber`.
    ///
    /// ```
    /// use music21_rs::metadata::{Metadata, MetadataQuery};
    /// use music21_rs::{Stream, StreamKind};
    ///
    /// let mut opus = Stream::with_kind(StreamKind::Opus);
    /// for number in ["1", "2"] {
    ///     let mut score = Stream::with_kind(StreamKind::Score);
    ///     let mut metadata = Metadata::new();
    ///     metadata.add_text("number", number);
    ///     metadata.add_text("title", format!("Tune {number}"));
    ///     score.set_metadata(Some(metadata));
    ///     opus.push(score);
    /// }
    ///
    /// let second = opus.score_by_number(&MetadataQuery::Text("2")).unwrap();
    /// assert_eq!(second.metadata().unwrap().title().as_deref(), Some("Tune 2"));
    /// ```
    pub fn score_by_number(&self, query: &MetadataQuery<'_>) -> Option<&Stream> {
        self.score_by_field(query, "number")
    }

    /// The first score whose title a query matches: music21's
    /// `Opus.getScoreByTitle`. music21 reads a regular expression out of a
    /// title query that holds one; here that is a
    /// [`MetadataQuery::Matcher`] the caller builds.
    pub fn score_by_title(&self, query: &MetadataQuery<'_>) -> Option<&Stream> {
        self.score_by_field(query, "title")
    }

    /// The first score whose metadata matches a query in one field.
    fn score_by_field(&self, query: &MetadataQuery<'_>, field: &str) -> Option<&Stream> {
        self.scores().into_iter().find(|score| {
            score
                .metadata()
                .is_some_and(|metadata| metadata.search(query, Some(field)).is_some())
        })
    }

    /// One score of the opus's scores laid out as its parts: music21's
    /// `Opus.mergeScores`, for a collection whose scores are each one part
    /// of the same work.
    ///
    /// Each score's first part is written out as notation and inserted at
    /// the start of the new score, which takes the first title and the
    /// first composer any of them names.
    ///
    /// # Errors
    ///
    /// A score holding no part, which music21 raises on as well, or a part
    /// that cannot be written as notation.
    pub fn merge_scores(&self) -> Result<Stream> {
        let mut merged = Stream::with_kind(StreamKind::Score);
        let mut metadata = Metadata::new();
        for score in self.scores() {
            let Some(part) = score.parts().first().copied() else {
                return Err(Error::Stream(
                    "an opus score has no part to merge".to_string(),
                ));
            };
            // music21 assumes the one part, and writes it out as notation.
            let mut part = part.clone();
            make_notation(&mut part)?;
            merged.insert(0.0, part);

            let Some(found) = score.metadata() else {
                continue;
            };
            // The first of each is kept, as music21 keeps it.
            if metadata.get("title").is_empty()
                && let Some(title) = found.string_value("title")
            {
                metadata.add_text("title", title);
            }
            if metadata.get("composer").is_empty()
                && let Some(composer) = found.string_value("composer")
            {
                metadata.add_contributor("composer", composer);
            }
        }
        merged.set_metadata(Some(metadata));
        Ok(merged)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::defaults::FloatType;
    use crate::metadata::MetadataQuery;
    use crate::note::Note;
    use crate::{Duration, TimeSignature};

    fn tune(number: &str, title: &str, composer: Option<&str>) -> Stream {
        let mut score = Stream::with_kind(StreamKind::Score);
        let mut metadata = Metadata::new();
        metadata.add_text("number", number);
        metadata.add_text("title", title);
        if let Some(composer) = composer {
            metadata.add_contributor("composer", composer);
        }
        score.set_metadata(Some(metadata));
        let mut part = Stream::with_kind(StreamKind::Part);
        part.insert(0.0, TimeSignature::new(2, 4).expect("2/4 is a meter"));
        for (place, name) in ["C4", "D4", "E4", "F4"].iter().enumerate() {
            let note = Note::from_name(name)
                .expect("a note name")
                .with_duration(Duration::eighth());
            part.insert(place as FloatType * 0.5, note);
        }
        score.push(part);
        score
    }

    fn opus() -> Stream {
        let mut opus = Stream::with_kind(StreamKind::Opus);
        opus.push(tune("1", "Vrienden, kommt alle gaere", Some("Anonymous")));
        opus.push(tune("2", "O Venus bant", None));
        opus
    }

    #[test]
    fn an_opus_names_the_numbers_its_scores_carry() {
        assert_eq!(
            opus().score_numbers(),
            [Some("1".to_string()), Some("2".to_string())]
        );
        assert_eq!(Stream::with_kind(StreamKind::Opus).score_numbers(), []);
    }

    #[test]
    fn a_score_is_found_by_its_number_and_by_its_title() {
        let opus = opus();
        let second = opus
            .score_by_number(&MetadataQuery::Text("2"))
            .expect("the second tune");
        assert_eq!(
            second.metadata().and_then(Metadata::title).as_deref(),
            Some("O Venus bant")
        );
        let first = opus
            .score_by_title(&MetadataQuery::Text("kommt"))
            .expect("the first tune");
        assert_eq!(
            first.metadata().and_then(Metadata::title).as_deref(),
            Some("Vrienden, kommt alle gaere")
        );
        assert!(opus.score_by_number(&MetadataQuery::Text("9")).is_none());
    }

    #[test]
    fn a_matcher_stands_in_for_music21s_regular_expression() {
        let opus = opus();
        let kommt = |text: &str| text.starts_with("Vrienden") && text.ends_with("gaere");
        let found = opus
            .score_by_title(&MetadataQuery::Matcher(&kommt))
            .expect("the first tune");
        assert_eq!(
            found.metadata().and_then(Metadata::title).as_deref(),
            Some("Vrienden, kommt alle gaere")
        );
    }

    #[test]
    fn merging_the_scores_makes_one_score_of_parts() {
        let merged = opus().merge_scores().expect("two parts to merge");
        assert_eq!(merged.kind(), StreamKind::Score);
        assert_eq!(merged.parts().len(), 2);
        let metadata = merged.metadata().expect("the merged metadata");
        assert_eq!(
            metadata.title().as_deref(),
            Some("Vrienden, kommt alle gaere")
        );
        assert_eq!(
            metadata.string_value("composer").as_deref(),
            Some("Anonymous")
        );
        // Each part was written out as notation: music21's makeNotation.
        assert_eq!(merged.parts()[0].measures().len(), 1);
    }

    #[test]
    fn a_score_without_a_part_cannot_be_merged() {
        let mut opus = Stream::with_kind(StreamKind::Opus);
        opus.push(Stream::with_kind(StreamKind::Score));
        assert!(opus.merge_scores().is_err());
    }
}
