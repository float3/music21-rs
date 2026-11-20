//! Metadata with what was found in the score it heads: music21's
//! `RichMetadata`, which its corpus cache keeps for every work.
//!
//! Beside a score's own metadata it holds thirteen values read off the
//! score itself -- the meters, keys and tempos in it, how many notes and
//! parts, its range, how long it is and where it lives in the corpus -- so
//! the corpus can be searched by them without reading any score.

use std::borrow::Cow;
use std::path::Path;

use super::{
    Metadata, contributor_answers_to, is_contributor_unique_name, namespace_name, old_work_id,
    unique_name_of_field,
};
use crate::common::numbertools::{OffsetQL, op_frac};
use crate::defaults::IntegerType;
use crate::error::Result;
use crate::interval::Interval;
use crate::stream::{Stream, StreamElement};

/// The names of the values read off the score: music21's
/// `additionalRichMetadataAttributes`, sorted.
pub const RICH_ATTRIBUTES: [&str; 13] = [
    "ambitus",
    "keySignatureFirst",
    "keySignatures",
    "noteCount",
    "numberOfParts",
    "pitchHighest",
    "pitchLowest",
    "scoreQuarterLength",
    "sourcePath",
    "tempoFirst",
    "tempos",
    "timeSignatureFirst",
    "timeSignatures",
];

/// A score's range as a few plain values: music21's `AmbitusShort`.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AmbitusShort {
    /// The span in semitones.
    pub semitones: IntegerType,
    /// The span's simple diatonic name, `P5`.
    pub diatonic: String,
    /// The lowest pitch, by name with octave.
    pub pitch_lowest: Option<String>,
    /// The highest pitch, by name with octave.
    pub pitch_highest: Option<String>,
}

/// One of the values read off a score, as a search compares it.
#[derive(Clone, Debug, PartialEq)]
pub enum RichValue<'a> {
    /// Nothing was found.
    None,
    /// A text: a meter, a tempo, a pitch, a path.
    Text(Cow<'a, str>),
    /// A count, or a key signature's sharps.
    Integer(IntegerType),
    /// A length in quarter notes.
    Length(OffsetQL),
    /// Every meter or tempo, in order.
    Texts(&'a [String]),
    /// Every key signature, as sharps, in order.
    Integers(&'a [IntegerType]),
    /// The range.
    Ambitus(&'a AmbitusShort),
}

/// What a [`RichMetadata::search`] asks of a value: music21's query, which
/// may be a string, a pattern, a function or a number.
pub enum RichQuery<'a> {
    /// Text held in a value's text, either case ignored.
    Text(&'a str),
    /// A pattern, answered for a value's text: music21's regular
    /// expression, which never matches what is not text.
    Pattern(&'a dyn Fn(&str) -> bool),
    /// A function of any value: music21's callable query.
    Function(&'a dyn Fn(&RichValue<'_>) -> bool),
    /// A number: held in a text as written, or equal to a count.
    Integer(IntegerType),
    /// A key signature: equal to a count of sharps.
    Sharps(IntegerType),
}

impl std::fmt::Debug for RichQuery<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Text(query) => formatter.debug_tuple("Text").field(query).finish(),
            Self::Pattern(_) => formatter.write_str("Pattern(..)"),
            Self::Function(_) => formatter.write_str("Function(..)"),
            Self::Integer(query) => formatter.debug_tuple("Integer").field(query).finish(),
            Self::Sharps(query) => formatter.debug_tuple("Sharps").field(query).finish(),
        }
    }
}

impl RichQuery<'_> {
    fn matches(&self, value: &RichValue<'_>) -> bool {
        match (self, value) {
            (Self::Function(function), value) => function(value),
            (Self::Text(query), RichValue::Text(text)) => {
                text.to_lowercase().contains(&query.to_lowercase())
            }
            (Self::Pattern(pattern), RichValue::Text(text)) => pattern(text),
            (Self::Integer(query), RichValue::Text(text)) => text.contains(&query.to_string()),
            (Self::Integer(query) | Self::Sharps(query), RichValue::Integer(value)) => {
                query == value
            }
            _ => false,
        }
    }
}

/// A score's metadata and what was read off the score: music21's
/// `RichMetadata`.
///
/// ```
/// use music21::metadata::rich::{RichMetadata, RichQuery};
/// use music21::tinynotation::from_tiny_notation;
///
/// let line = from_tiny_notation("3/4 c4 d e g a b")?;
/// let mut rich = RichMetadata::default();
/// rich.update(&line, None)?;
/// assert_eq!(rich.time_signatures, ["3/4"]);
/// assert_eq!(rich.note_count, Some(6));
/// assert_eq!(rich.search(&RichQuery::Text("3/4"), None), Some(Some("timeSignatureFirst")));
/// # Ok::<(), music21::Error>(())
/// ```
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RichMetadata {
    /// The score's own metadata.
    pub metadata: Metadata,
    /// The range of its pitches.
    pub ambitus: Option<AmbitusShort>,
    /// The sharps of its first key signature.
    pub key_signature_first: Option<IntegerType>,
    /// The sharps of every key signature, each once, in order.
    pub key_signatures: Vec<IntegerType>,
    /// How many notes, chords and rests it has.
    pub note_count: Option<IntegerType>,
    /// How many parts it has.
    pub number_of_parts: Option<IntegerType>,
    /// Its highest pitch, by name with octave.
    pub pitch_highest: Option<String>,
    /// Its lowest pitch, by name with octave.
    pub pitch_lowest: Option<String>,
    /// How long it is, in quarter notes.
    pub score_quarter_length: OffsetQL,
    /// Where it lives in the corpus, a POSIX path.
    pub source_path: String,
    /// Its first tempo, as music21 writes it.
    pub tempo_first: Option<String>,
    /// Every tempo, each once, in order.
    pub tempos: Vec<String>,
    /// Its first meter, `3/4`.
    pub time_signature_first: Option<String>,
    /// Every meter, each once, in order.
    pub time_signatures: Vec<String>,
}

impl Default for RichMetadata {
    fn default() -> Self {
        Self {
            metadata: Metadata::new(),
            ambitus: None,
            key_signature_first: None,
            key_signatures: Vec::new(),
            note_count: None,
            number_of_parts: None,
            pitch_highest: None,
            pitch_lowest: None,
            score_quarter_length: OffsetQL::Float(0.0),
            source_path: String::new(),
            tempo_first: None,
            tempos: Vec::new(),
            time_signature_first: None,
            time_signatures: Vec::new(),
        }
    }
}

impl RichMetadata {
    /// Rich metadata over a score's own.
    pub fn of(metadata: Metadata) -> Self {
        Self {
            metadata,
            ..Self::default()
        }
    }

    /// One of the values read off the score, by its name in
    /// [`RICH_ATTRIBUTES`].
    pub fn value(&self, name: &str) -> Option<RichValue<'_>> {
        fn text(found: &Option<String>) -> RichValue<'_> {
            found
                .as_deref()
                .map_or(RichValue::None, |text| RichValue::Text(Cow::Borrowed(text)))
        }
        let integer =
            |found: Option<IntegerType>| found.map_or(RichValue::None, RichValue::Integer);
        let value = match name {
            "ambitus" => self
                .ambitus
                .as_ref()
                .map_or(RichValue::None, RichValue::Ambitus),
            "keySignatureFirst" => integer(self.key_signature_first),
            "keySignatures" => RichValue::Integers(&self.key_signatures),
            "noteCount" => integer(self.note_count),
            "numberOfParts" => integer(self.number_of_parts),
            "pitchHighest" => text(&self.pitch_highest),
            "pitchLowest" => text(&self.pitch_lowest),
            "scoreQuarterLength" => RichValue::Length(self.score_quarter_length),
            "sourcePath" => RichValue::Text(Cow::Borrowed(&self.source_path)),
            "tempoFirst" => text(&self.tempo_first),
            "tempos" => RichValue::Texts(&self.tempos),
            "timeSignatureFirst" => text(&self.time_signature_first),
            "timeSignatures" => RichValue::Texts(&self.time_signatures),
            _ => return None,
        };
        Some(value)
    }

    /// Reads the thirteen values off a score: music21's `update`.
    /// `corpus` is the corpus directory, which a score's file path is
    /// written relative to where it holds no corpus path of its own.
    ///
    /// # Errors
    ///
    /// A span of pitches no interval names.
    pub fn update(&mut self, stream: &Stream, corpus: Option<&Path>) -> Result<()> {
        self.number_of_parts = Some(stream.parts().len() as IntegerType);
        self.source_path = source_path(stream, corpus);
        self.key_signatures.clear();
        self.tempos.clear();
        self.time_signatures.clear();

        // One pass over the flattened score gathers meters, keys and tempos.
        let flat = stream.flatten();
        for (_, element) in flat.leaves() {
            match element {
                StreamElement::TimeSignature(meter) => {
                    push_new(&mut self.time_signatures, meter.ratio_string());
                }
                StreamElement::KeySignature(signature) => {
                    push_new(&mut self.key_signatures, signature.sharps());
                }
                StreamElement::Key(key) => push_new(&mut self.key_signatures, key.sharps()),
                StreamElement::MetronomeMark(mark) => {
                    push_new(&mut self.tempos, mark.music21_repr());
                }
                StreamElement::TempoText(text) => push_new(&mut self.tempos, text.music21_repr()),
                _ => {}
            }
        }
        self.time_signature_first = self.time_signatures.first().cloned();
        self.key_signature_first = self.key_signatures.first().copied();
        self.tempo_first = self.tempos.first().cloned();
        self.note_count = Some(flat.notes_and_rests().len() as IntegerType);
        self.score_quarter_length = op_frac(crate::variant::highest_time(&flat))?;

        // The range, P1 where there are no pitches.
        let span = crate::analysis::stream_pitch_span(stream);
        self.pitch_lowest = span.as_ref().map(|(low, _)| low.name_with_octave());
        self.pitch_highest = span.as_ref().map(|(_, high)| high.name_with_octave());
        let interval = match &span {
            Some((low, high)) => Interval::between_pitches(low, high)?,
            None => Interval::from_name("P1")?,
        };
        self.ambitus = Some(AmbitusShort {
            semitones: interval.semitones() as IntegerType,
            diatonic: interval.diatonic().simple_name(),
            pitch_lowest: self.pitch_lowest.clone(),
            pitch_highest: self.pitch_highest.clone(),
        });
        Ok(())
    }

    /// The first value a query matches, and the field it was found under,
    /// searching the score's values beside its metadata: music21's
    /// `RichMetadata.search`. `None` where nothing matched; `Some(None)`
    /// for a contributor with no role.
    pub fn search(&self, query: &RichQuery<'_>, field: Option<&str>) -> Option<Option<&str>> {
        let field = field.map(str::to_lowercase);
        self.search_pairs(field.as_deref())
            .into_iter()
            .find(|(value, _)| query.matches(value))
            .map(|(_, field)| field)
    }

    /// The values a search looks through and the field each answers under,
    /// in music21's order.
    fn search_pairs(&self, field: Option<&str>) -> Vec<(RichValue<'_>, Option<&str>)> {
        let mut pairs = Vec::new();
        match field {
            Some(field) => pairs.extend(self.field_pairs(field)),
            None => pairs.extend(
                self.standard_values()
                    .into_iter()
                    .map(|(name, value)| (value, Some(name))),
            ),
        }

        // Every contributor's name, under its role.
        for (_, value) in self.metadata.contributors() {
            if !contributor_answers_to(value.role(), field) {
                continue;
            }
            let names: Vec<&str> = match value.as_contributor() {
                Some(contributor) => contributor.names().iter().map(|name| name.data()).collect(),
                None => vec![value.text()],
            };
            for name in names {
                pairs.push((RichValue::Text(Cow::Borrowed(name)), value.role()));
            }
        }
        pairs
    }

    /// What a search of one field looks through: the values of the name it
    /// spells out, or else the first standard name holding it.
    fn field_pairs(&self, field: &str) -> Vec<(RichValue<'_>, Option<&str>)> {
        // music21 lowercases the field first, so only the two rich names
        // written in lower case are found whole.
        if let Some(name) = RICH_ATTRIBUTES.iter().find(|name| **name == field) {
            let value = self.value(name).unwrap_or(RichValue::None);
            if value != RichValue::None {
                return vec![(value, Some(*name))];
            }
        }
        if let Some((unique_name, asked)) = unique_name_of_field(field) {
            let texts = self.metadata.field_texts(unique_name);
            if !texts.is_empty() {
                return texts
                    .into_iter()
                    .map(|text| (RichValue::Text(text), Some(asked)))
                    .collect();
            }
        }

        // The first standard name holding the field, or whose old work id
        // does, with that one value.
        for (name, value) in self.standard_values() {
            if name.to_lowercase().contains(field) {
                return vec![(value, Some(name))];
            }
            let Some(work_id) = old_work_id(name) else {
                continue;
            };
            if work_id.to_lowercase().contains(field) {
                return vec![(value, Some(work_id))];
            }
        }
        Vec::new()
    }

    /// Every standard value that is not a contributor's, the score's values
    /// among them, sorted by name: music21's `all(skipContributors=True)`
    /// without the names a caller made up.
    fn standard_values(&self) -> Vec<(&str, RichValue<'_>)> {
        let mut values: Vec<(&str, RichValue<'_>)> = self
            .metadata
            .all_sorted()
            .into_iter()
            .filter(|(name, _)| !is_contributor_unique_name(name) && namespace_name(name).is_some())
            .map(|(name, value)| (name, RichValue::Text(Cow::Borrowed(value.text()))))
            .collect();
        for name in RICH_ATTRIBUTES {
            values.push((name, self.value(name).unwrap_or(RichValue::None)));
        }
        values.sort_by(|left, right| left.0.cmp(right.0));
        values
    }
}

/// Adds a value not there yet.
fn push_new<T: PartialEq>(values: &mut Vec<T>, value: T) {
    if !values.contains(&value) {
        values.push(value);
    }
}

/// Where a score lives in the corpus: its metadata's corpus path, or its
/// file path relative to the corpus, as POSIX: music21's `getSourcePath`.
fn source_path(stream: &Stream, corpus: Option<&Path>) -> String {
    let Some(metadata) = stream.metadata() else {
        return String::new();
    };
    if let Some(found) = metadata
        .first_text("corpusFilePath")
        .filter(|found| !found.is_empty())
    {
        return found.to_string();
    }
    let Some(file) = metadata
        .first_text("filePath")
        .filter(|file| !file.is_empty())
    else {
        return String::new();
    };
    let file = Path::new(file);
    let relative = corpus
        .and_then(|corpus| file.strip_prefix(corpus).ok())
        .unwrap_or(file);
    relative
        .components()
        .map(|part| part.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
        .replacen("//", "/", 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bach() -> RichMetadata {
        let mut metadata = Metadata::new();
        metadata.add_contributor("composer", "J.S. Bach");
        metadata.add_text("title", "Aus meines Herzens Grunde");
        let mut rich = RichMetadata::of(metadata);
        rich.note_count = Some(12);
        rich.time_signatures = vec!["3/4".to_string()];
        rich.time_signature_first = Some("3/4".to_string());
        rich.source_path = "bach/bwv1.6.mxl".to_string();
        rich
    }

    #[test]
    fn a_search_reads_the_score_s_values() {
        let rich = bach();
        assert_eq!(
            rich.search(&RichQuery::Text("3/4"), None),
            Some(Some("timeSignatureFirst"))
        );
        assert_eq!(
            rich.search(&RichQuery::Text("bach"), Some("composer")),
            Some(Some("composer"))
        );
        assert_eq!(
            rich.search(&RichQuery::Text("bwv1.6"), None),
            Some(Some("sourcePath"))
        );
        assert_eq!(rich.search(&RichQuery::Text("mozart"), None), None);
    }

    #[test]
    fn a_camel_case_field_is_found_by_its_lowercase_spelling() {
        let rich = bach();
        let fewer =
            |value: &RichValue<'_>| matches!(value, RichValue::Integer(count) if *count < 20);
        let query = RichQuery::Function(&fewer);
        assert_eq!(
            rich.search(&query, Some("noteCount")),
            Some(Some("noteCount"))
        );
        assert_eq!(
            rich.search(&RichQuery::Integer(12), Some("notecount")),
            Some(Some("noteCount"))
        );
    }

    #[test]
    fn a_list_is_not_text() {
        let rich = bach();
        assert_eq!(
            rich.search(&RichQuery::Text("3/4"), Some("timeSignatures")),
            None
        );
    }
}
