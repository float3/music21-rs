//! The metadata of many scores, searched and cached together: music21's
//! `metadata.bundles`.
//!
//! A [`MetadataBundle`] maps a key made from each score's corpus path to a
//! [`MetadataEntry`]: where the score is and its [`RichMetadata`]. music21
//! keeps the bundle of each corpus as a gzipped pickle; [`MetadataBundle::
//! from_cache`] reads the pickle music21 writes and [`MetadataBundle::
//! to_cache`] writes one music21 reads. Compressing is the caller's.
//!
//! ```
//! use music21::metadata::Metadata;
//! use music21::metadata::bundles::{MetadataBundle, MetadataEntry};
//! use music21::metadata::rich::{RichMetadata, RichQuery};
//!
//! let mut metadata = Metadata::new();
//! metadata.add_contributor("composer", "Arcangelo Corelli");
//! let mut bundle = MetadataBundle::new(None);
//! bundle.insert(MetadataEntry::new("corelli/opus3no1/1grave.xml", None, Some(RichMetadata::of(metadata))));
//!
//! let found = bundle.search(&RichQuery::Text("corelli"), Some("composer"), &[]);
//! assert_eq!(found.keys(), ["corelli_opus3no1_1grave_xml"]);
//!
//! let read = MetadataBundle::from_cache(&bundle.to_cache())?;
//! assert_eq!(read.len(), 1);
//! # Ok::<(), music21::Error>(())
//! ```

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::path::{Component, Path};
use std::sync::Arc;

use super::primitives::{Contributor, Date, DateKind, DatePart, DatePrimitive, Relevance, Text};
use super::rich::{AmbitusShort, RichMetadata, RichQuery};
use super::{Metadata, MetadataContent, MetadataValue};
use crate::common::numbertools::OffsetQL;
use crate::common::pickle::{Construction, Pickle, PickleValue, ValueId};
use crate::defaults::IntegerType;
use crate::error::{Error, Result};

/// The module music21 defines bundles in, which its cache names.
const BUNDLES_MODULE: &str = "music21.metadata.bundles";
/// The module music21 defines `RichMetadata` and `AmbitusShort` in.
const METADATA_MODULE: &str = "music21.metadata";
/// The module music21 defines its metadata primitives in.
const PRIMITIVES_MODULE: &str = "music21.metadata.primitives";

/// How a path was given to [`corpus_path_to_key`]: music21 reads a
/// `pathlib.Path` by its parts and a string by its text.
#[derive(Clone, Copy, Debug)]
pub enum KeySource<'a> {
    /// A path, and the corpus directory it is read relative to where it
    /// lies inside it.
    Path {
        /// The path.
        path: &'a Path,
        /// music21's `common.getSourceFilePath() / 'corpus'`.
        corpus: Option<&'a Path>,
    },
    /// A string.
    Text(&'a str),
}

/// The key a score is filed under in a bundle: its corpus path with every
/// separator and dot an underscore, and its number in an opus after it:
/// music21's `MetadataBundle.corpusPathToKey`.
///
/// ```
/// use music21::metadata::bundles::{KeySource, corpus_path_to_key};
///
/// let key = corpus_path_to_key(KeySource::Text("corelli/opus3no1/1grave.xml"), Some("3"));
/// assert_eq!(key, "corelli_opus3no1_1grave_xml_3");
/// ```
pub fn corpus_path_to_key(source: KeySource<'_>, number: Option<&str>) -> String {
    let path = match source {
        KeySource::Path { path, corpus } => {
            let relative = corpus
                .and_then(|corpus| path.strip_prefix(corpus).ok())
                .unwrap_or(path);
            let mut parts: Vec<String> = relative
                .components()
                .map(|part| match part {
                    Component::RootDir => "/".to_string(),
                    other => other.as_os_str().to_string_lossy().into_owned(),
                })
                .collect();
            if parts.len() > 1 && parts[0] == "/" {
                parts.remove(0);
            }
            parts.join("_")
        }
        KeySource::Text(text) => {
            let after = text.rsplit("corpus").next().unwrap_or(text);
            let after = after
                .strip_prefix(std::path::MAIN_SEPARATOR)
                .unwrap_or(after);
            after.replace(['/', std::path::MAIN_SEPARATOR], "_")
        }
    };
    let key = path.replace('.', "_");
    match number {
        Some(number) => format!("{key}_{number}"),
        None => key,
    }
}

/// One score in a bundle: music21's `MetadataEntry`.
#[derive(Clone, Debug, PartialEq)]
pub struct MetadataEntry {
    /// Where the score is, usually relative to the corpus directory.
    pub source_path: String,
    /// Which score of an opus it is, where it is one.
    pub number: Option<String>,
    /// Its metadata; nothing for the stand-in a whole opus is filed under.
    pub metadata: Option<RichMetadata>,
    /// The corpus it came from.
    pub corpus_name: String,
}

impl MetadataEntry {
    /// An entry of no named corpus.
    pub fn new(
        source_path: impl Into<String>,
        number: Option<String>,
        metadata: Option<RichMetadata>,
    ) -> Self {
        Self {
            source_path: source_path.into(),
            number,
            metadata,
            corpus_name: String::new(),
        }
    }

    /// The key it is filed under: music21's `corpusPath`, which reads the
    /// source path as a `pathlib.Path`.
    pub fn corpus_path(&self, corpus: Option<&Path>) -> String {
        let source = KeySource::Path {
            path: Path::new(&self.source_path),
            corpus,
        };
        corpus_path_to_key(source, self.number.as_deref())
    }

    /// Its source path's parts, which entries are sorted by, as
    /// `pathlib.Path`s compare.
    fn path_parts(&self) -> Vec<&str> {
        self.source_path
            .split('/')
            .filter(|part| !part.is_empty())
            .collect()
    }

    /// Its source path's suffix with the dot, `.xml`.
    fn suffix(&self) -> &str {
        let name = self.source_path.rsplit('/').next().unwrap_or("");
        match name.rfind('.') {
            Some(at) if at > 0 && at + 1 < name.len() => &name[at..],
            _ => "",
        }
    }
}

/// Which set of keys a combination of two bundles keeps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SetOperation {
    /// Keys in either.
    Union,
    /// Keys in both.
    Intersection,
    /// Keys in the first alone.
    Difference,
    /// Keys in one alone.
    SymmetricDifference,
}

/// The metadata of many scores, filed by key in the order they were
/// filed: music21's `MetadataBundle`.
///
/// Entries are shared, not copied, between a bundle and what is searched
/// or combined out of it, so two bundles hold the same entries where
/// music21's would hold the same objects.
#[derive(Clone, Debug, Default)]
pub struct MetadataBundle {
    name: Option<String>,
    entries: Vec<(String, Arc<MetadataEntry>)>,
    index: HashMap<String, usize>,
}

impl PartialEq for MetadataBundle {
    /// The same entries under the same keys in the same order, as music21
    /// compares two bundles' `OrderedDict`s of the same objects.
    fn eq(&self, other: &Self) -> bool {
        self.entries.len() == other.entries.len()
            && self
                .entries
                .iter()
                .zip(&other.entries)
                .all(|(left, right)| left.0 == right.0 && Arc::ptr_eq(&left.1, &right.1))
    }
}

impl MetadataBundle {
    /// An empty bundle, of the corpus `name` or of none.
    pub fn new(name: Option<String>) -> Self {
        Self {
            name,
            ..Self::default()
        }
    }

    /// The corpus it belongs to: `core`, `local` or a local corpus's name.
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// How many entries it has.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether it has none.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Its keys, in order.
    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(key, _)| key.as_str()).collect()
    }

    /// The entry at a place, with its key.
    pub fn get(&self, at: usize) -> Option<(&str, &Arc<MetadataEntry>)> {
        self.entries
            .get(at)
            .map(|(key, entry)| (key.as_str(), entry))
    }

    /// The entry filed under a key.
    pub fn entry(&self, key: &str) -> Option<&Arc<MetadataEntry>> {
        self.index.get(key).map(|at| &self.entries[*at].1)
    }

    /// Every entry with its key, in order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &Arc<MetadataEntry>)> {
        self.entries
            .iter()
            .map(|(key, entry)| (key.as_str(), entry))
    }

    /// Files an entry under its corpus path, in place of one already there.
    pub fn insert(&mut self, entry: MetadataEntry) {
        let key = entry.corpus_path(None);
        self.insert_shared(key, Arc::new(entry));
    }

    /// Files a shared entry under a key, in place of one already there.
    pub fn insert_shared(&mut self, key: String, entry: Arc<MetadataEntry>) {
        if let Some(at) = self.index.get(&key) {
            self.entries[*at].1 = entry;
            return;
        }
        self.index.insert(key.clone(), self.entries.len());
        self.entries.push((key, entry));
    }

    /// Removes every entry: music21's `clear`.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.index.clear();
    }

    /// Keeps the entries whose score `exists` finds, answering how many
    /// went: music21's `validate`, which leaves finding files to its
    /// caller here.
    pub fn validate(&mut self, exists: impl Fn(&str) -> bool) -> usize {
        let before = self.entries.len();
        let mut known: HashMap<String, bool> = HashMap::new();
        self.entries.retain(|(_, entry)| {
            *known.entry(entry.source_path.clone()).or_insert_with(|| {
                entry.source_path.starts_with("http:") || exists(&entry.source_path)
            })
        });
        self.reindex();
        before - self.entries.len()
    }

    fn reindex(&mut self) {
        self.index = self
            .entries
            .iter()
            .enumerate()
            .map(|(at, (key, _))| (key.clone(), at))
            .collect();
    }

    /// Sorts the entries by source path, keeping the order of those that
    /// share one.
    fn sort_by_path(&mut self) {
        self.entries
            .sort_by(|left, right| left.1.path_parts().cmp(&right.1.path_parts()));
        self.reindex();
    }

    /// The entries whose metadata a query matches, sorted by source path,
    /// in a bundle of no corpus: music21's `search`. `extensions`, where
    /// given, are the suffixes kept, dots and all.
    pub fn search(
        &self,
        query: &RichQuery<'_>,
        field: Option<&str>,
        extensions: &[String],
    ) -> MetadataBundle {
        let mut found = MetadataBundle::new(None);
        for (key, entry) in &self.entries {
            let Some(metadata) = &entry.metadata else {
                continue;
            };
            if !extensions.is_empty() && !extensions.iter().any(|kept| kept == entry.suffix()) {
                continue;
            }
            if found.index.contains_key(key) {
                continue;
            }
            if metadata.search(query, field).is_some() {
                found.insert_shared(key.clone(), Arc::clone(entry));
            }
        }
        found.sort_by_path();
        found
    }

    /// The entries of the keys an operation keeps, from this bundle where
    /// it has them, sorted by source path, in a bundle of no corpus.
    pub fn combine(&self, other: &MetadataBundle, operation: SetOperation) -> MetadataBundle {
        let mine: HashSet<&str> = self.index.keys().map(String::as_str).collect();
        let theirs: HashSet<&str> = other.index.keys().map(String::as_str).collect();
        let keeps = |key: &str| match operation {
            SetOperation::Union => true,
            SetOperation::Intersection => mine.contains(key) && theirs.contains(key),
            SetOperation::Difference => !theirs.contains(key),
            SetOperation::SymmetricDifference => mine.contains(key) != theirs.contains(key),
        };
        let mut result = MetadataBundle::new(None);
        for (key, entry) in self.entries.iter().chain(&other.entries) {
            if keeps(key) && !result.index.contains_key(key) {
                result.insert_shared(key.clone(), Arc::clone(entry));
            }
        }
        result.sort_by_path();
        result
    }

    /// How this bundle's keys stand to another's: `Less` for a proper
    /// subset, `Equal` for the same keys, `Greater` for a proper superset,
    /// and nothing where neither holds the other.
    pub fn compare_keys(&self, other: &MetadataBundle) -> Option<Ordering> {
        let subset = self.index.keys().all(|key| other.index.contains_key(key));
        let superset = other.index.keys().all(|key| self.index.contains_key(key));
        match (subset, superset) {
            (true, true) => Some(Ordering::Equal),
            (true, false) => Some(Ordering::Less),
            (false, true) => Some(Ordering::Greater),
            (false, false) => None,
        }
    }

    /// Whether no key is in both.
    pub fn is_disjoint(&self, other: &MetadataBundle) -> bool {
        !self.index.keys().any(|key| other.index.contains_key(key))
    }

    /// Reads a bundle from the pickle music21 caches one as.
    ///
    /// # Errors
    ///
    /// Bytes that are no pickle, or a pickle of anything but a bundle.
    pub fn from_cache(bytes: &[u8]) -> Result<Self> {
        let pickle = Pickle::read(bytes)?;
        CacheReader { pickle: &pickle }.bundle()
    }

    /// The pickle music21 caches a bundle as, protocol 5: music21 reads it
    /// with `pickle.load`.
    pub fn to_cache(&self) -> Vec<u8> {
        let mut writer = CacheWriter::default();
        let root = writer.bundle(self);
        writer.pickle.set_root(root);
        writer.pickle.write()
    }
}

fn cache_error(message: impl Into<String>) -> Error {
    Error::Metadata(format!("metadata cache: {}", message.into()))
}

/// Reads music21's pickled objects into the crate's.
struct CacheReader<'a> {
    pickle: &'a Pickle,
}

impl CacheReader<'_> {
    fn text(&self, id: Option<ValueId>) -> Option<String> {
        id.and_then(|id| self.pickle.as_text(id))
            .map(str::to_string)
    }

    fn integer(&self, id: Option<ValueId>) -> Option<IntegerType> {
        match id.map(|id| self.pickle.get(id)) {
            Some(PickleValue::Int(value)) => Some(*value as IntegerType),
            _ => None,
        }
    }

    fn bundle(&self) -> Result<MetadataBundle> {
        let root = self.pickle.root();
        if self.pickle.class_of(root) != Some((BUNDLES_MODULE, "MetadataBundle")) {
            return Err(cache_error("not a pickled MetadataBundle"));
        }
        let name = self.text(self.pickle.dict_get(root, "_name"));
        let mut bundle = MetadataBundle::new(name);
        let Some(entries) = self.pickle.dict_get(root, "_metadataEntries") else {
            return Ok(bundle);
        };
        for (key, entry) in self.pickle.dict_items(entries) {
            let Some(key) = self.pickle.as_text(*key) else {
                continue;
            };
            bundle.insert_shared(key.to_string(), Arc::new(self.entry(*entry)?));
        }
        Ok(bundle)
    }

    fn entry(&self, id: ValueId) -> Result<MetadataEntry> {
        let field = |name| self.pickle.dict_get(id, name);
        let source_path = self.text(field("_sourcePath")).unwrap_or_default();
        let number = match field("_number").map(|number| self.pickle.get(number)) {
            Some(PickleValue::Text(text)) => Some(text.clone()),
            Some(PickleValue::Int(value)) => Some(value.to_string()),
            _ => None,
        };
        let metadata = match field("_metadataPayload") {
            Some(payload) if self.pickle.class_of(payload).is_some() => Some(self.rich(payload)?),
            _ => None,
        };
        Ok(MetadataEntry {
            source_path,
            number,
            metadata,
            corpus_name: self.text(field("_corpusName")).unwrap_or_default(),
        })
    }

    fn rich(&self, id: ValueId) -> Result<RichMetadata> {
        let field = |name| self.pickle.dict_get(id, name);
        let mut metadata = Metadata::new();
        if let Some(contents) = field("_contents") {
            for (name, values) in self.pickle.dict_items(contents) {
                let Some(name) = self.pickle.as_text(*name) else {
                    continue;
                };
                for value in self.pickle.members(*values) {
                    if let Some(value) = self.value(*value)? {
                        metadata.add(name, value);
                    }
                }
            }
        }
        let texts = |name| {
            field(name).map_or_else(Vec::new, |list| {
                self.pickle
                    .members(list)
                    .iter()
                    .filter_map(|id| self.pickle.as_text(*id).map(str::to_string))
                    .collect()
            })
        };
        let key_signatures = field("keySignatures").map_or_else(Vec::new, |list| {
            self.pickle
                .members(list)
                .iter()
                .filter_map(|id| self.integer(Some(*id)))
                .collect()
        });
        Ok(RichMetadata {
            metadata,
            ambitus: field("ambitus").and_then(|ambitus| self.ambitus(ambitus)),
            key_signature_first: self.integer(field("keySignatureFirst")),
            key_signatures,
            note_count: self.integer(field("noteCount")),
            number_of_parts: self.integer(field("numberOfParts")),
            pitch_highest: self.text(field("pitchHighest")),
            pitch_lowest: self.text(field("pitchLowest")),
            score_quarter_length: field("scoreQuarterLength")
                .map_or(OffsetQL::Float(0.0), |length| self.length(length)),
            source_path: self.text(field("sourcePath")).unwrap_or_default(),
            tempo_first: self.text(field("tempoFirst")),
            tempos: texts("tempos"),
            time_signature_first: self.text(field("timeSignatureFirst")),
            time_signatures: texts("timeSignatures"),
        })
    }

    fn ambitus(&self, id: ValueId) -> Option<AmbitusShort> {
        let args = self.pickle.args_of(id);
        if args.len() < 4 {
            return None;
        }
        Some(AmbitusShort {
            semitones: self.integer(Some(args[0]))?,
            diatonic: self.text(Some(args[1]))?,
            pitch_lowest: self.text(Some(args[2])),
            pitch_highest: self.text(Some(args[3])),
        })
    }

    fn length(&self, id: ValueId) -> OffsetQL {
        match self.pickle.get(id) {
            PickleValue::Float(value) => OffsetQL::Float(*value),
            PickleValue::Int(value) => OffsetQL::Float(*value as f64),
            PickleValue::Object { .. } => {
                // `Fraction` pickles as `Fraction('5077/12')`.
                let written = self
                    .text(self.pickle.args_of(id).first().copied())
                    .unwrap_or_default();
                let (numerator, denominator) = written.split_once('/').unwrap_or((&written, "1"));
                OffsetQL::Fraction {
                    numerator: numerator.parse().unwrap_or(0),
                    denominator: denominator.parse().unwrap_or(1),
                }
            }
            _ => OffsetQL::Float(0.0),
        }
    }

    /// One of music21's primitives as a value: a `Text`, `Copyright`,
    /// `Contributor` or one of the four dates. Anything else is left out.
    fn value(&self, id: ValueId) -> Result<Option<MetadataValue>> {
        let Some((_, class)) = self.pickle.class_of(id) else {
            return Ok(self.pickle.as_text(id).map(MetadataValue::new));
        };
        let value = match class {
            "Text" | "Copyright" => MetadataValue::of_text(self.text_primitive(id)),
            "Contributor" => {
                let mut contributor = Contributor::unnamed();
                contributor.set_role(self.text(self.pickle.dict_get(id, "_role")));
                let names = self
                    .pickle
                    .dict_get(id, "_names")
                    .map_or_else(Vec::new, |names| {
                        self.pickle
                            .members(names)
                            .iter()
                            .map(|name| self.text_primitive(*name))
                            .collect()
                    });
                contributor.set_names(names);
                contributor.set_birth(self.date_field(id, "birth")?);
                contributor.set_death(self.date_field(id, "death")?);
                MetadataValue::of_contributor(contributor)
            }
            "DateSingle" | "DateRelative" | "DateBetween" | "DateSelection" => {
                MetadataValue::of_date(self.date(id)?)
            }
            _ => return Ok(None),
        };
        Ok(Some(value))
    }

    fn text_primitive(&self, id: ValueId) -> Text {
        let field = |name| self.pickle.dict_get(id, name);
        let mut text = Text::new(self.text(field("_data")).unwrap_or_default());
        text.set_language(self.text(field("_language")));
        text.set_encoding_scheme(self.text(field("encodingScheme")));
        if let Some(PickleValue::Bool(translated)) =
            field("isTranslated").map(|id| self.pickle.get(id))
        {
            text.set_translated(Some(*translated));
        }
        if self.pickle.class_of(id).map(|(_, class)| class) == Some("Copyright") {
            text.set_role(self.text(field("role")));
        }
        text
    }

    fn date_field(&self, id: ValueId, name: &str) -> Result<Option<DatePrimitive>> {
        match self.pickle.dict_get(id, name) {
            Some(date) if self.pickle.class_of(date).is_some() => Ok(Some(self.date(date)?)),
            _ => Ok(None),
        }
    }

    fn date(&self, id: ValueId) -> Result<DatePrimitive> {
        let kind = match self.pickle.class_of(id).map(|(_, class)| class) {
            Some("DateRelative") => DateKind::Relative,
            Some("DateBetween") => DateKind::Between,
            Some("DateSelection") => DateKind::Selection,
            _ => DateKind::Single,
        };
        let relevance = self
            .text(self.pickle.dict_get(id, "_relevance"))
            .and_then(|name| Relevance::named(&name))
            .unwrap_or_else(|| kind.default_relevance());
        let dates = self
            .pickle
            .dict_get(id, "_data")
            .map_or_else(Vec::new, |dates| {
                self.pickle
                    .members(dates)
                    .iter()
                    .map(|date| self.one_date(*date))
                    .collect()
            });
        DatePrimitive::of_dates(dates, relevance, kind)
    }

    fn one_date(&self, id: ValueId) -> Date {
        let mut date = Date::new();
        for part in DatePart::ALL {
            let value = self.integer(self.pickle.dict_get(id, part.name()));
            let error = self
                .text(self.pickle.dict_get(id, &format!("{}Error", part.name())))
                .and_then(|name| super::primitives::Certainty::named(&name));
            date.set_unchecked(part, value, error);
        }
        date
    }
}

/// Writes the crate's objects as music21 pickles its own.
#[derive(Default)]
struct CacheWriter {
    pickle: Pickle,
    none: Option<ValueId>,
}

impl CacheWriter {
    fn none(&mut self) -> ValueId {
        if let Some(none) = self.none {
            return none;
        }
        let none = self.pickle.push(PickleValue::None);
        self.none = Some(none);
        none
    }

    fn optional_text(&mut self, text: Option<&str>) -> ValueId {
        match text {
            Some(text) => self.pickle.text(text),
            None => self.none(),
        }
    }

    fn optional_integer(&mut self, value: Option<IntegerType>) -> ValueId {
        match value {
            Some(value) => self.pickle.push(PickleValue::Int(value as i64)),
            None => self.none(),
        }
    }

    /// An object of a class of music21's, made with `NEWOBJ` and given a
    /// `__dict__`.
    fn object(&mut self, module: &str, class: &str, state: Vec<(&str, ValueId)>) -> ValueId {
        let class = self.pickle.global(module, class);
        let items = state
            .into_iter()
            .map(|(name, value)| (self.pickle.text(name), value))
            .collect();
        let state = self.pickle.push(PickleValue::Dict(items));
        self.pickle.push(PickleValue::Object {
            class,
            construction: Construction::New,
            args: Vec::new(),
            state: Some(state),
            items: Vec::new(),
            entries: Vec::new(),
        })
    }

    fn list(&mut self, members: Vec<ValueId>) -> ValueId {
        self.pickle.push(PickleValue::List(members))
    }

    fn bundle(&mut self, bundle: &MetadataBundle) -> ValueId {
        let ordered = self.pickle.global("collections", "OrderedDict");
        let mut entries = Vec::with_capacity(bundle.len());
        for (key, entry) in bundle.iter() {
            let key = self.pickle.text(key);
            entries.push((key, self.entry(entry)));
        }
        let ordered = self.pickle.push(PickleValue::Object {
            class: ordered,
            construction: Construction::Call,
            args: Vec::new(),
            state: None,
            items: Vec::new(),
            entries,
        });
        let corpus = self.none();
        let name = self.optional_text(bundle.name());
        self.object(
            BUNDLES_MODULE,
            "MetadataBundle",
            vec![
                ("_metadataEntries", ordered),
                ("_corpus", corpus),
                ("_name", name),
            ],
        )
    }

    fn entry(&mut self, entry: &MetadataEntry) -> ValueId {
        let source_path = self.pickle.text(&entry.source_path);
        let number = self.optional_text(entry.number.as_deref());
        let payload = match &entry.metadata {
            Some(rich) => self.rich(rich),
            None => self.none(),
        };
        let corpus_name = self.pickle.text(&entry.corpus_name);
        self.object(
            BUNDLES_MODULE,
            "MetadataEntry",
            vec![
                ("_sourcePath", source_path),
                ("_number", number),
                ("_metadataPayload", payload),
                ("_corpusName", corpus_name),
            ],
        )
    }

    /// `RichMetadata`, with the `Music21Object` attributes music21 pickles
    /// for an element in no stream.
    fn rich(&mut self, rich: &RichMetadata) -> ValueId {
        let mut state = self.element_state();
        let contents = self.contents(&rich.metadata);
        state.push(("_contents", contents));

        let ambitus = match &rich.ambitus {
            Some(ambitus) => self.ambitus(ambitus),
            None => self.none(),
        };
        state.push(("ambitus", ambitus));
        let key_first = self.optional_integer(rich.key_signature_first);
        state.push(("keySignatureFirst", key_first));
        let keys = rich
            .key_signatures
            .iter()
            .map(|sharps| self.pickle.push(PickleValue::Int(*sharps as i64)))
            .collect();
        let keys = self.list(keys);
        state.push(("keySignatures", keys));
        let notes = self.optional_integer(rich.note_count);
        state.push(("noteCount", notes));
        let parts = self.optional_integer(rich.number_of_parts);
        state.push(("numberOfParts", parts));
        let highest = self.optional_text(rich.pitch_highest.as_deref());
        state.push(("pitchHighest", highest));
        let lowest = self.optional_text(rich.pitch_lowest.as_deref());
        state.push(("pitchLowest", lowest));
        let length = self.length(rich.score_quarter_length);
        state.push(("scoreQuarterLength", length));
        let source = self.pickle.text(&rich.source_path);
        state.push(("sourcePath", source));
        let tempo_first = self.optional_text(rich.tempo_first.as_deref());
        state.push(("tempoFirst", tempo_first));
        let tempos = self.texts(&rich.tempos);
        state.push(("tempos", tempos));
        let meter_first = self.optional_text(rich.time_signature_first.as_deref());
        state.push(("timeSignatureFirst", meter_first));
        let meters = self.texts(&rich.time_signatures);
        state.push(("timeSignatures", meters));
        self.object(METADATA_MODULE, "RichMetadata", state)
    }

    fn texts(&mut self, texts: &[String]) -> ValueId {
        let members = texts.iter().map(|text| self.pickle.text(text)).collect();
        self.list(members)
    }

    /// What `Music21Object.__getstate__` gives for an element in no
    /// stream, as music21's cache holds it.
    fn element_state(&mut self) -> Vec<(&'static str, ValueId)> {
        let none = self.none();
        let offset = self.pickle.push(PickleValue::Float(0.0));
        let priority = self.pickle.push(PickleValue::Int(0));
        let cache = self.pickle.push(PickleValue::Dict(Vec::new()));
        let groups_class = self.pickle.global("music21.base", "Groups");
        let groups = self.pickle.push(PickleValue::Object {
            class: groups_class,
            construction: Construction::New,
            args: Vec::new(),
            state: None,
            items: Vec::new(),
            entries: Vec::new(),
        });
        let sites = self.sites();
        vec![
            ("_id", none),
            ("_activeSite", none),
            ("_naiveOffset", offset),
            ("_activeSiteStoredOffset", none),
            ("_derivation", none),
            ("_style", none),
            ("_editorial", none),
            ("_duration", none),
            ("_priority", priority),
            ("_cache", cache),
            ("groups", groups),
            ("sites", sites),
        ]
    }

    /// `Sites` holding only the dead reference to no site that every
    /// element starts with.
    fn sites(&mut self) -> ValueId {
        let none = self.none();
        let minus_two = self.pickle.push(PickleValue::Int(-2));
        let dead = self.pickle.push(PickleValue::Bool(true));
        let site_ref = self.object(
            "music21.sites",
            "SiteRef",
            vec![
                ("siteWeakref", none),
                ("siteIndex", minus_two),
                ("globalSiteIndex", minus_two),
                ("classString", none),
                ("isDead", dead),
            ],
        );
        let ordered = self.pickle.global("collections", "OrderedDict");
        let site_dict = self.pickle.push(PickleValue::Object {
            class: ordered,
            construction: Construction::Call,
            args: Vec::new(),
            state: None,
            items: Vec::new(),
            entries: vec![(none, site_ref)],
        });
        let index = self.pickle.push(PickleValue::Int(0));
        let last = self.pickle.push(PickleValue::Int(-1));
        self.object(
            "music21.sites",
            "Sites",
            vec![
                ("_siteIndex", index),
                ("siteDict", site_dict),
                ("_lastID", last),
            ],
        )
    }

    fn contents(&mut self, metadata: &Metadata) -> ValueId {
        let mut names: Vec<(&str, Vec<ValueId>)> = Vec::new();
        for (name, value) in metadata.all() {
            let written = self.value(value);
            match names.iter_mut().find(|(found, _)| *found == name) {
                Some((_, values)) => values.push(written),
                None => names.push((name, vec![written])),
            }
        }
        let items = names
            .into_iter()
            .map(|(name, values)| (self.pickle.text(name), self.list(values)))
            .collect();
        self.pickle.push(PickleValue::Dict(items))
    }

    fn value(&mut self, value: &MetadataValue) -> ValueId {
        match value.content() {
            MetadataContent::Text(text) => self.text_primitive(text),
            MetadataContent::Contributor(contributor) => self.contributor(contributor),
            MetadataContent::Date(date) => self.date(date),
        }
    }

    fn text_primitive(&mut self, text: &Text) -> ValueId {
        let data = self.pickle.text(text.data());
        let language = self.optional_text(text.language());
        let translated = match text.is_translated() {
            Some(translated) => self.pickle.push(PickleValue::Bool(translated)),
            None => self.none(),
        };
        let scheme = self.optional_text(text.encoding_scheme());
        let mut state = vec![
            ("_data", data),
            ("_language", language),
            ("isTranslated", translated),
            ("encodingScheme", scheme),
        ];
        let class = match text.role() {
            Some(role) => {
                let role = self.pickle.text(role);
                state.push(("role", role));
                "Copyright"
            }
            None => "Text",
        };
        self.object(PRIMITIVES_MODULE, class, state)
    }

    fn contributor(&mut self, contributor: &Contributor) -> ValueId {
        let role = self.optional_text(contributor.role());
        let names = contributor
            .names()
            .iter()
            .map(|name| self.text_primitive(name))
            .collect();
        let names = self.list(names);
        let nationality = self.list(Vec::new());
        let birth = match contributor.birth() {
            Some(date) => self.date(date),
            None => self.none(),
        };
        let death = match contributor.death() {
            Some(date) => self.date(date),
            None => self.none(),
        };
        self.object(
            PRIMITIVES_MODULE,
            "Contributor",
            vec![
                ("_role", role),
                ("_names", names),
                ("_nationality", nationality),
                ("birth", birth),
                ("death", death),
            ],
        )
    }

    fn date(&mut self, date: &DatePrimitive) -> ValueId {
        let dates: Vec<ValueId> = date.dates().iter().map(|one| self.one_date(one)).collect();
        let count = dates.len();
        let dates = self.list(dates);
        let relevance = self.pickle.text(date.relevance().name());

        // A single date's certainty is its relevance; the others have none.
        let uncertainty = match date.kind() {
            DateKind::Single => vec![self.pickle.text(date.relevance().name())],
            _ => vec![self.none(); count],
        };
        let uncertainty = self.list(uncertainty);
        self.object(
            PRIMITIVES_MODULE,
            date.kind().name(),
            vec![
                ("_data", dates),
                ("_relevance", relevance),
                ("_dataUncertainty", uncertainty),
            ],
        )
    }

    fn one_date(&mut self, date: &Date) -> ValueId {
        let mut state = Vec::new();
        for part in DatePart::ALL {
            let value = self.optional_integer(date.value(part));
            state.push((part.name().to_string(), value));
        }
        for part in DatePart::ALL {
            let error = self.optional_text(date.certainty(part).map(|certainty| certainty.name()));
            state.push((format!("{}Error", part.name()), error));
        }
        let names = DatePart::ALL
            .iter()
            .map(|part| self.pickle.text(part.name()))
            .collect();
        let names = self.pickle.push(PickleValue::Tuple(names));
        state.push(("attrNames".to_string(), names));
        let class = self.pickle.global(PRIMITIVES_MODULE, "Date");
        let items = state
            .into_iter()
            .map(|(name, value)| (self.pickle.text(&name), value))
            .collect();
        let state = self.pickle.push(PickleValue::Dict(items));
        self.pickle.push(PickleValue::Object {
            class,
            construction: Construction::New,
            args: Vec::new(),
            state: Some(state),
            items: Vec::new(),
            entries: Vec::new(),
        })
    }

    fn ambitus(&mut self, ambitus: &AmbitusShort) -> ValueId {
        let class = self.pickle.global(METADATA_MODULE, "AmbitusShort");
        let semitones = self.pickle.push(PickleValue::Int(ambitus.semitones as i64));
        let diatonic = self.pickle.text(&ambitus.diatonic);
        let lowest = self.optional_text(ambitus.pitch_lowest.as_deref());
        let highest = self.optional_text(ambitus.pitch_highest.as_deref());
        self.pickle.push(PickleValue::Object {
            class,
            construction: Construction::New,
            args: vec![semitones, diatonic, lowest, highest],
            state: None,
            items: Vec::new(),
            entries: Vec::new(),
        })
    }

    fn length(&mut self, length: OffsetQL) -> ValueId {
        match length {
            OffsetQL::Float(value) => self.pickle.push(PickleValue::Float(value)),
            OffsetQL::Fraction {
                numerator,
                denominator,
            } => {
                let class = self.pickle.global("fractions", "Fraction");
                let written = self.pickle.text(&format!("{numerator}/{denominator}"));
                self.pickle.push(PickleValue::Object {
                    class,
                    construction: Construction::Call,
                    args: vec![written],
                    state: None,
                    items: Vec::new(),
                    entries: Vec::new(),
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(path: &str, composer: &str, meter: &str) -> MetadataEntry {
        let mut metadata = Metadata::new();
        metadata.add_contributor("composer", composer);
        let mut rich = RichMetadata::of(metadata);
        rich.time_signature_first = Some(meter.to_string());
        rich.time_signatures = vec![meter.to_string()];
        rich.score_quarter_length = OffsetQL::Fraction {
            numerator: 5077,
            denominator: 12,
        };
        MetadataEntry::new(path, None, Some(rich))
    }

    fn bundle() -> MetadataBundle {
        let mut bundle = MetadataBundle::new(Some("core".to_string()));
        bundle.insert(entry("bach/bwv7.7.mxl", "J.S. Bach", "4/4"));
        bundle.insert(entry("bach/bwv11.6.mxl", "J.S. Bach", "3/4"));
        bundle.insert(entry(
            "corelli/opus3no1/1grave.xml",
            "Arcangelo Corelli",
            "4/4",
        ));
        bundle.insert(MetadataEntry::new("airdsAirs/book1.abc", None, None));
        bundle
    }

    #[test]
    fn keys_read_strings_and_paths_as_music21_does() {
        // music21 strips only the platform's own separator after "corpus".
        let sep = std::path::MAIN_SEPARATOR;
        let written = format!("{sep}music21{sep}corpus{sep}bach{sep}bwv1007{sep}prelude");
        let text = KeySource::Text(&written);
        assert_eq!(corpus_path_to_key(text, None), "bach_bwv1007_prelude");
        let path = KeySource::Path {
            path: Path::new("/m21/corpus/bach/bwv66.6.mxl"),
            corpus: Some(Path::new("/m21/corpus")),
        };
        assert_eq!(corpus_path_to_key(path, Some("2")), "bach_bwv66_6_mxl_2");
        let outside = KeySource::Path {
            path: Path::new("/tmp/myFile.xml"),
            corpus: None,
        };
        assert_eq!(corpus_path_to_key(outside, None), "tmp_myFile_xml");
    }

    #[test]
    fn a_search_skips_stubs_and_sorts_by_path() {
        let bach = bundle().search(&RichQuery::Text("bach"), Some("composer"), &[]);
        assert_eq!(bach.keys(), ["bach_bwv11_6_mxl", "bach_bwv7_7_mxl"]);
        assert_eq!(bach.name(), None);
        let xml = [".xml".to_string()];
        assert_eq!(
            bundle().search(&RichQuery::Text("4/4"), None, &xml).len(),
            1
        );
    }

    #[test]
    fn set_operations_work_on_keys() {
        let core = bundle();
        let bach = core.search(&RichQuery::Text("bach"), Some("composer"), &[]);
        let triple = core.search(&RichQuery::Text("3/4"), None, &[]);
        assert_eq!(bach.combine(&triple, SetOperation::Intersection).len(), 1);
        assert_eq!(bach.combine(&triple, SetOperation::Difference).len(), 1);
        assert_eq!(
            bach.combine(&triple, SetOperation::SymmetricDifference)
                .len(),
            1
        );
        assert_eq!(bach.combine(&core, SetOperation::Union).len(), 4);
        assert_eq!(triple.compare_keys(&bach), Some(Ordering::Less));
        assert!(!bach.is_disjoint(&triple));
        assert_eq!(
            bach,
            core.search(&RichQuery::Text("bach"), Some("composer"), &[])
        );
    }

    #[test]
    fn the_cache_reads_back_what_it_writes() {
        let core = bundle();
        let read = MetadataBundle::from_cache(&core.to_cache()).unwrap();
        assert_eq!(read.name(), Some("core"));
        assert_eq!(read.keys(), core.keys());
        let (_, first) = read.get(0).unwrap();
        assert_eq!(**first, **core.get(0).unwrap().1);
        assert!(
            read.entry("airdsAirs_book1_abc")
                .unwrap()
                .metadata
                .is_none()
        );
    }

    #[test]
    fn validate_drops_what_is_not_there() {
        let mut core = bundle();
        assert_eq!(core.validate(|path| path.starts_with("bach")), 2);
        assert_eq!(core.keys(), ["bach_bwv7_7_mxl", "bach_bwv11_6_mxl"]);
    }
}
