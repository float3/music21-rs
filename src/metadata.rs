//! A score's metadata: music21's `metadata.Metadata`.
//!
//! music21 keeps metadata as named values, each name a *unique name* from a
//! fixed vocabulary drawn from Dublin Core, the MARC relator terms and
//! Humdrum's reference records, or one a caller made up. A name may carry
//! several values, and the names keep the order they were first given in,
//! which is the order a MusicXML writer puts them out in.
//!
//! A contributor is a value with a role: the composer, an arranger, a
//! lyricist. Its unique name is its role where the role is in the
//! vocabulary, and `otherContributor` where it is not.

use std::borrow::Cow;

/// One value of a metadata name: its text and, for a contributor or a
/// copyright, the role it was given.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MetadataValue {
    text: String,
    role: Option<String>,
}

impl MetadataValue {
    /// A value that is only text.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            role: None,
        }
    }

    /// A value with a role, as a contributor or a copyright has.
    pub fn with_role(text: impl Into<String>, role: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            role: Some(role.into()),
        }
    }

    /// The value as text: music21's `str(value)`.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The role, where the value has one.
    pub fn role(&self) -> Option<&str> {
        self.role.as_deref()
    }
}

/// A score's metadata: named values in the order the names were first given.
///
/// ```
/// use music21_rs::metadata::Metadata;
///
/// let mut md = Metadata::new();
/// md.add_text("title", "Wer nur den lieben Gott");
/// md.add_contributor("composer", "J. S. Bach");
/// assert_eq!(md.first_text("title"), Some("Wer nur den lieben Gott"));
/// assert_eq!(md.contributors().next().map(|(_, value)| value.text()), Some("J. S. Bach"));
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[must_use]
pub struct Metadata {
    entries: Vec<(String, Vec<MetadataValue>)>,
}

impl Metadata {
    /// Metadata with nothing in it.
    ///
    /// music21's own constructor starts with its version under `software`;
    /// this one starts empty, and a caller who wants a software line adds it.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a value under a unique name, after any it already has.
    pub fn add(&mut self, unique_name: impl Into<String>, value: MetadataValue) {
        let unique_name = unique_name.into();
        match self
            .entries
            .iter_mut()
            .find(|(name, _)| *name == unique_name)
        {
            Some((_, values)) => values.push(value),
            None => self.entries.push((unique_name, vec![value])),
        }
    }

    /// Adds a value that is only text.
    pub fn add_text(&mut self, unique_name: impl Into<String>, text: impl Into<String>) {
        self.add(unique_name, MetadataValue::new(text));
    }

    /// Adds a contributor in a role: under the role itself where it is one
    /// of the vocabulary's contributor names, and under `otherContributor`
    /// where it is not, as music21's `addContributor` files one.
    pub fn add_contributor(&mut self, role: impl Into<String>, name: impl Into<String>) {
        let role = role.into();
        let unique_name = if is_contributor_unique_name(&role) {
            role.clone()
        } else {
            "otherContributor".to_string()
        };
        self.add(unique_name, MetadataValue::with_role(name, role));
    }

    /// Replaces every value under a unique name.
    pub fn set(&mut self, unique_name: impl Into<String>, values: Vec<MetadataValue>) {
        let unique_name = unique_name.into();
        self.entries.retain(|(name, _)| *name != unique_name);
        if !values.is_empty() {
            self.entries.push((unique_name, values));
        }
    }

    /// Every value under a unique name, in the order they were added.
    pub fn get(&self, unique_name: &str) -> &[MetadataValue] {
        self.entries
            .iter()
            .find(|(name, _)| name == unique_name)
            .map_or(&[], |(_, values)| values.as_slice())
    }

    /// The text of the first value under a unique name.
    pub fn first_text(&self, unique_name: &str) -> Option<&str> {
        self.get(unique_name).first().map(MetadataValue::text)
    }

    /// Every name and value in order, names in the order they were first
    /// given and each name's values together: music21's `all` with
    /// `returnSorted=False`.
    pub fn all(&self) -> impl Iterator<Item = (&str, &MetadataValue)> {
        self.entries
            .iter()
            .flat_map(|(name, values)| values.iter().map(move |value| (name.as_str(), value)))
    }

    /// The contributors alone, in the same order.
    pub fn contributors(&self) -> impl Iterator<Item = (&str, &MetadataValue)> {
        self.all()
            .filter(|(name, _)| is_contributor_unique_name(name))
    }

    /// Every value under a unique name as one piece of text, as music21's
    /// attribute of that name reads it -- `title`, `composer`, `movementName`:
    /// one contributor by name, two joined with `and`, more as the first
    /// `and` how many others; a title's values each with an article written
    /// after a comma put back in front; anything else the values joined with
    /// commas. Nothing for a name outside the vocabulary, or with no value.
    pub fn string_value(&self, unique_name: &str) -> Option<String> {
        namespace_name(unique_name)?;
        let values = self.get(unique_name);
        if values.is_empty() {
            return None;
        }
        if is_contributor_unique_name(unique_name) {
            return Some(match values {
                [only] => only.text().to_string(),
                [first, second] => format!("{} and {}", first.text(), second.text()),
                [first, rest @ ..] => format!("{} and {} others", first.text(), rest.len()),
                [] => String::new(),
            });
        }
        let texts: Vec<String> = if ARTICLED_TITLES.contains(&unique_name) {
            values
                .iter()
                .map(|value| {
                    crate::text::prepend_article(value.text(), None)
                        .unwrap_or_else(|_| value.text().to_string())
                })
                .collect()
        } else {
            values
                .iter()
                .map(|value| value.text().to_string())
                .collect()
        };
        Some(texts.join(", "))
    }

    /// The title, as [`Self::string_value`] reads it: music21's `title`.
    pub fn title(&self) -> Option<String> {
        self.string_value("title")
    }

    /// Whether nothing is in it.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Every name and value sorted by name and then by text, as music21's
    /// `all` reads them by default and [`Self::search`] walks them.
    pub fn all_sorted(&self) -> Vec<(&str, &MetadataValue)> {
        let mut all: Vec<(&str, &MetadataValue)> = self.all().collect();
        all.sort_by(|(left, left_value), (right, right_value)| {
            left.cmp(right)
                .then_with(|| left_value.text().cmp(right_value.text()))
        });
        all
    }

    /// The first value a query matches, and the field it was found under:
    /// music21's `search`.
    ///
    /// With no `field`, every standard name is searched, sorted by name,
    /// and then every contributor in the order they were added, under its
    /// role rather than its unique name so that a made-up role can be
    /// searched for.
    ///
    /// A `field` names where to look, and need not be the whole name:
    /// music21 takes a unique name, one of its four grandfathered work ids
    /// (`date`, `volume`, `dedication`, `commission`), one of its twenty
    /// Humdrum abbreviations (`otl`, `ode`), or any piece of a name --
    /// `compos` finds `composer`. A field naming nothing finds nothing.
    ///
    /// ```
    /// use music21_rs::metadata::{Metadata, MetadataQuery};
    ///
    /// let mut md = Metadata::new();
    /// md.add_contributor("composer", "Joplin, Scott");
    /// md.add_text("title", "Maple Leaf Rag");
    ///
    /// let joplin = MetadataQuery::Text("joplin");
    /// assert_eq!(
    ///     md.search(&joplin, Some("composer")).unwrap().field(),
    ///     Some("composer")
    /// );
    /// assert_eq!(
    ///     md.search(&joplin, Some("compos")).unwrap().field(),
    ///     Some("composer")
    /// );
    /// assert!(md.search(&joplin, Some("title")).is_none());
    ///
    /// let found = md.search(&MetadataQuery::Text("leaf"), None).unwrap();
    /// assert_eq!((found.field(), found.text()), (Some("title"), "Maple Leaf Rag"));
    /// ```
    pub fn search(
        &self,
        query: &MetadataQuery<'_>,
        field: Option<&str>,
    ) -> Option<MetadataMatch<'_>> {
        let field = field.map(str::to_lowercase);
        self.search_fields(field.as_deref())
            .into_iter()
            .find(|(text, _)| query.matches(text))
            .map(|(text, field)| MetadataMatch { field, text })
    }

    /// The text a search looks through and the field each piece answers
    /// under, in the order music21 builds them.
    fn search_fields(&self, field: Option<&str>) -> Vec<(Cow<'_, str>, Option<&str>)> {
        let mut pairs: Vec<(Cow<'_, str>, Option<&str>)> = Vec::new();
        match field {
            Some(field) => {
                let named = unique_name_of_field(field);
                let texts =
                    named.map_or_else(Vec::new, |(unique_name, _)| self.field_texts(unique_name));
                match (texts.is_empty(), named) {
                    // music21 answers under the field as it was asked for,
                    // not under the unique name that field stands for.
                    (false, Some((_, asked))) => {
                        pairs.extend(texts.into_iter().map(|text| (text, Some(asked))));
                    }
                    _ => pairs.extend(self.partial_field_match(field)),
                }
            }
            None => pairs.extend(
                self.standard_non_contributors()
                    .map(|(name, value)| (Cow::Borrowed(value.text()), Some(name))),
            ),
        }
        pairs.extend(
            self.contributors()
                .filter(|(_, value)| contributor_answers_to(value.role(), field))
                .map(|(_, value)| (Cow::Borrowed(value.text()), value.role())),
        );
        pairs
    }

    /// A name's values as a search of that name alone reads them: a
    /// title-like name's article written back in front, which music21's
    /// `_getStringValuesByNamespaceName` does and the rest of a search
    /// does not, so that a title stored as `Grassy Turf, The` is searched
    /// as `The Grassy Turf` under `title` and as itself under every field.
    fn field_texts(&self, unique_name: &str) -> Vec<Cow<'_, str>> {
        self.get(unique_name)
            .iter()
            .map(|value| {
                if ARTICLED_TITLES.contains(&unique_name) {
                    Cow::Owned(
                        crate::text::prepend_article(value.text(), None)
                            .unwrap_or_else(|_| value.text().to_string()),
                    )
                } else {
                    Cow::Borrowed(value.text())
                }
            })
            .collect()
    }

    /// The vocabulary's names that are not contributors, sorted, with their
    /// values: music21's `all(skipContributors=True)` without the names a
    /// caller made up, which it does not search.
    fn standard_non_contributors(&self) -> impl Iterator<Item = (&str, &MetadataValue)> {
        self.all_sorted()
            .into_iter()
            .filter(|(name, _)| !is_contributor_unique_name(name) && namespace_name(name).is_some())
    }

    /// The first standard name holding `field` anywhere in it, with the
    /// first of that name's values: music21's partial field match, which
    /// stops at the name it finds and searches that one value alone.
    fn partial_field_match(&self, field: &str) -> Option<(Cow<'_, str>, Option<&str>)> {
        self.standard_non_contributors().find_map(|(name, value)| {
            if name.to_lowercase().contains(field) {
                return Some((Cow::Borrowed(value.text()), Some(name)));
            }
            let work_id = old_work_id(name)?;
            work_id
                .to_lowercase()
                .contains(field)
                .then_some((Cow::Borrowed(value.text()), Some(work_id)))
        })
    }
}

/// Whether a contributor answers to a search field: every contributor where
/// none was asked for, one whose role holds the field, and a contributor
/// with no role only under `contributor` itself.
fn contributor_answers_to(role: Option<&str>, field: Option<&str>) -> bool {
    let Some(field) = field else {
        return true;
    };
    match role {
        None => field == "contributor",
        Some(role) => role.to_lowercase().contains(field),
    }
}

/// The unique name an attribute name stands for: the name itself, one of
/// music21's four grandfathered work ids, or one of its twenty Humdrum
/// abbreviations. Nothing for a name outside the vocabulary.
///
/// Spelling counts, as it does for music21's own attribute access: `otl`
/// and `title` both name the title, `OTL` names nothing.
///
/// ```
/// use music21_rs::metadata::unique_name_of;
///
/// assert_eq!(unique_name_of("title"), Some("title"));
/// assert_eq!(unique_name_of("otl"), Some("title"));
/// assert_eq!(unique_name_of("dedication"), Some("dedicatedTo"));
/// assert_eq!(unique_name_of("OTL"), None);
/// assert_eq!(unique_name_of("madeUp"), None);
/// ```
pub fn unique_name_of(name: &str) -> Option<&'static str> {
    if let Some((found, _, _)) = STANDARD_PROPERTIES
        .iter()
        .find(|(unique, _, _)| *unique == name)
    {
        return Some(found);
    }
    OLD_MUSIC21_NAMES
        .iter()
        .find(|(_, abbreviation, work_id)| *abbreviation == Some(name) || *work_id == Some(name))
        .map(|(unique, _, _)| *unique)
}

/// The unique name a search field stands for where it names one outright,
/// with the spelling it was asked for: the name itself, one of music21's
/// grandfathered work ids, or one of its Humdrum abbreviations. The field
/// reaching here is already lowercase, so a name written with a capital in
/// it is only found by [`Metadata::partial_field_match`], as in music21.
fn unique_name_of_field(field: &str) -> Option<(&'static str, &'static str)> {
    if let Some((name, _, _)) = STANDARD_PROPERTIES
        .iter()
        .find(|(name, _, _)| *name == field)
    {
        return Some((name, name));
    }
    OLD_MUSIC21_NAMES
        .iter()
        .find_map(
            |(name, abbreviation, work_id)| match (abbreviation, work_id) {
                (Some(asked), _) if *asked == field => Some((*name, *asked)),
                (_, Some(asked)) if *asked == field => Some((*name, *asked)),
                _ => None,
            },
        )
}

/// The work id a unique name was written as before music21 v8, where it had
/// one: music21's `UNIQUE_NAME_TO_MUSIC21_WORK_ID`.
fn old_work_id(unique_name: &str) -> Option<&'static str> {
    OLD_MUSIC21_NAMES
        .iter()
        .find(|(name, _, _)| *name == unique_name)
        .and_then(|(_, _, work_id)| *work_id)
}

/// What a [`Metadata::search`] asks of one value's text.
///
/// music21 reads a regular expression out of a plain string where it holds
/// any of `*.|+?{}`, and matches the rest by containment. Nothing here
/// compiles a pattern -- the crate carries no regular expression engine --
/// so [`Self::Text`] is music21's plain-string reading alone, and a caller
/// who wants its regular expression hands over a [`Self::Matcher`] built
/// from whichever engine they already have.
pub enum MetadataQuery<'a> {
    /// A value matches where its text holds this text, either case
    /// ignored: music21's string query.
    Text(&'a str),
    /// A value matches where this answers `true` to its text: music21's
    /// callable query.
    Matcher(&'a dyn Fn(&str) -> bool),
}

impl MetadataQuery<'_> {
    /// Whether one value's text matches.
    fn matches(&self, text: &str) -> bool {
        match self {
            Self::Text(query) => text.to_lowercase().contains(&query.to_lowercase()),
            Self::Matcher(matcher) => matcher(text),
        }
    }
}

impl std::fmt::Debug for MetadataQuery<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Text(query) => formatter.debug_tuple("Text").field(query).finish(),
            Self::Matcher(_) => formatter.write_str("Matcher(..)"),
        }
    }
}

/// What a [`Metadata::search`] found: the text that matched and the field it
/// answers under. A contributor with a made-up role answers under that
/// role, and one with no role at all under no field, which is music21's
/// `(True, None)`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MetadataMatch<'a> {
    field: Option<&'a str>,
    text: Cow<'a, str>,
}

impl<'a> MetadataMatch<'a> {
    /// The field the value was found under.
    pub fn field(&self) -> Option<&'a str> {
        self.field
    }

    /// The text that matched.
    pub fn text(&self) -> &str {
        &self.text
    }
}

/// The unique names music21 kept an older spelling of: its
/// `oldMusic21Abbrev` and `oldMusic21WorkId`, both of which
/// [`Metadata::search`] takes as a field name. Each row is the unique
/// name, its Humdrum abbreviation and the work id it was written as
/// before music21 v8.
pub const OLD_MUSIC21_NAMES: [(&str, Option<&str>, Option<&str>); 21] = [
    ("alternativeTitle", Some("ota"), None),
    ("dateCreated", None, Some("date")),
    ("title", Some("otl"), None),
    ("textOriginalLanguage", Some("txo"), None),
    ("textLanguage", Some("txl"), None),
    ("popularTitle", Some("otp"), None),
    ("parentTitle", Some("opr"), None),
    ("actNumber", Some("oac"), None),
    ("sceneNumber", Some("osc"), None),
    ("movementNumber", Some("omv"), None),
    ("movementName", Some("omd"), None),
    ("opusNumber", Some("ops"), None),
    ("number", Some("onm"), None),
    ("volumeNumber", Some("ovm"), Some("volume")),
    ("dedicatedTo", Some("ode"), Some("dedication")),
    ("commissionedBy", Some("oco"), Some("commission")),
    ("countryOfComposition", Some("ocy"), None),
    ("localeOfComposition", Some("opc"), None),
    ("groupTitle", Some("gtl"), None),
    ("associatedWork", Some("gaw"), None),
    ("collectionDesignation", Some("gco"), None),
];

/// The names whose values are titles, an article at their end moved to the
/// front when read as text: music21's `needsArticleNormalization`.
const ARTICLED_TITLES: [&str; 6] = [
    "title",
    "alternativeTitle",
    "popularTitle",
    "parentTitle",
    "movementName",
    "groupTitle",
];

/// Whether a unique name is one of the vocabulary's contributor names:
/// music21's `_isContributorUniqueName`.
pub fn is_contributor_unique_name(unique_name: &str) -> bool {
    STANDARD_PROPERTIES
        .iter()
        .any(|(name, _, contributor)| *contributor && *name == unique_name)
}

/// The namespaced name a unique name stands for, `humdrum:OPC` for
/// `localeOfComposition`: music21's `uniqueNameToNamespaceName`. A name
/// outside the vocabulary has none.
pub fn namespace_name(unique_name: &str) -> Option<&'static str> {
    STANDARD_PROPERTIES
        .iter()
        .find(|(name, _, _)| *name == unique_name)
        .map(|(_, namespaced, _)| *namespaced)
}

/// music21's `STANDARD_PROPERTY_DESCRIPTIONS`: each property's unique name,
/// its namespaced name, and whether it names a contributor.
pub const STANDARD_PROPERTIES: [(&str, &str, bool); 131] = [
    ("abstract", "dcterms:abstract", false),
    ("accessRights", "dcterms:accessRights", false),
    ("alternativeTitle", "dcterms:alternative", false),
    ("audience", "dcterms:audience", false),
    ("dateAvailable", "dcterms:available", false),
    (
        "bibliographicCitation",
        "dcterms:bibliographicCitation",
        false,
    ),
    ("conformsTo", "dcterms:conformsTo", false),
    ("dateCreated", "dcterms:created", false),
    ("otherDate", "dcterms:date", false),
    ("dateAccepted", "dcterms:dateAccepted", false),
    ("dateCopyrighted", "dcterms:dateCopyrighted", false),
    ("dateSubmitted", "dcterms:dateSubmitted", false),
    ("description", "dcterms:description", false),
    ("educationLevel", "dcterms:educationLevel", false),
    ("extent", "dcterms:extent", false),
    ("format", "dcterms:format", false),
    ("hasFormat", "dcterms:hasFormat", false),
    ("hasPart", "dcterms:hasPart", false),
    ("hasVersion", "dcterms:hasVersion", false),
    ("identifier", "dcterms:identifier", false),
    ("instructionalMethod", "dcterms:instructionalMethod", false),
    ("isFormatOf", "dcterms:isFormatOf", false),
    ("isPartOf", "dcterms:isPartOf", false),
    ("isReferencedBy", "dcterms:isReferencedBy", false),
    ("isReplacedBy", "dcterms:isReplacedBy", false),
    ("isRequiredBy", "dcterms:isRequiredBy", false),
    ("dateIssued", "dcterms:issued", false),
    ("isVersionOf", "dcterms:isVersionOf", false),
    ("language", "dcterms:language", false),
    ("license", "dcterms:license", false),
    ("medium", "dcterms:medium", false),
    ("dateModified", "dcterms:modified", false),
    ("provenance", "dcterms:provenance", false),
    ("publisher", "dcterms:publisher", true),
    ("references", "dcterms:references", false),
    ("relation", "dcterms:relation", false),
    ("replaces", "dcterms:replaces", false),
    ("requires", "dcterms:requires", false),
    ("copyright", "dcterms:rights", false),
    ("rightsHolder", "dcterms:rightsHolder", true),
    ("source", "dcterms:source", false),
    ("subject", "dcterms:subject", false),
    ("tableOfContents", "dcterms:tableOfContents", false),
    ("title", "dcterms:title", false),
    ("type", "dcterms:type", false),
    ("dateValid", "dcterms:valid", false),
    ("adapter", "marcrel:ADP", true),
    ("analyst", "marcrel:ANL", true),
    ("annotator", "marcrel:ANN", true),
    ("arranger", "marcrel:ARR", true),
    ("quotationsAuthor", "marcrel:AQT", true),
    ("afterwordAuthor", "marcrel:AFT", true),
    ("dialogAuthor", "marcrel:AUD", true),
    ("introductionAuthor", "marcrel:AUI", true),
    ("calligrapher", "marcrel:CLL", true),
    ("collaborator", "marcrel:CLB", true),
    ("collotyper", "marcrel:CLT", true),
    ("commentaryAuthor", "marcrel:CWT", true),
    ("compiler", "marcrel:COM", true),
    ("composer", "marcrel:CMP", true),
    ("conceptor", "marcrel:CCP", true),
    ("conductor", "marcrel:CND", true),
    ("otherContributor", "marcrel:CTB", true),
    ("editor", "marcrel:EDT", true),
    ("engraver", "marcrel:EGR", true),
    ("etcher", "marcrel:ETR", true),
    ("illuminator", "marcrel:ILU", true),
    ("illustrator", "marcrel:ILL", true),
    ("instrumentalist", "marcrel:ITR", true),
    ("librettist", "marcrel:LBT", true),
    ("lithographer", "marcrel:LTG", true),
    ("lyricist", "marcrel:LYR", true),
    ("metalEngraver", "marcrel:MTE", true),
    ("musician", "marcrel:MUS", true),
    ("proofreader", "marcrel:PFR", true),
    ("platemaker", "marcrel:PLT", true),
    ("printmaker", "marcrel:PRM", true),
    ("producer", "marcrel:PRO", true),
    ("responsibleParty", "marcrel:RPY", true),
    ("scribe", "marcrel:SCR", true),
    ("singer", "marcrel:SNG", true),
    ("transcriber", "marcrel:TRC", true),
    ("translator", "marcrel:TRL", true),
    ("woodEngraver", "marcrel:WDE", true),
    ("woodCutter", "marcrel:WDC", true),
    ("accompanyingMaterialWriter", "marcrel:WAM", true),
    ("distributor", "marcrel:DST", true),
    ("software", "musicxml:software", false),
    ("textOriginalLanguage", "humdrum:TXO", false),
    ("textLanguage", "humdrum:TXL", false),
    ("popularTitle", "humdrum:OTP", false),
    ("parentTitle", "humdrum:OPR", false),
    ("actNumber", "humdrum:OAC", false),
    ("sceneNumber", "humdrum:OSC", false),
    ("movementNumber", "humdrum:OMV", false),
    ("movementName", "humdrum:OMD", false),
    ("opusNumber", "humdrum:OPS", false),
    ("number", "humdrum:ONM", false),
    ("volumeNumber", "humdrum:OVM", false),
    ("dedicatedTo", "humdrum:ODE", false),
    ("commissionedBy", "humdrum:OCO", false),
    ("countryOfComposition", "humdrum:OCY", false),
    ("localeOfComposition", "humdrum:OPC", false),
    ("groupTitle", "humdrum:GTL", false),
    ("associatedWork", "humdrum:GAW", false),
    ("collectionDesignation", "humdrum:GCO", false),
    ("attributedComposer", "humdrum:COA", true),
    ("suspectedComposer", "humdrum:COS", true),
    ("composerAlias", "humdrum:COL", true),
    ("composerCorporate", "humdrum:COC", true),
    ("orchestrator", "humdrum:LOR", true),
    ("firstPublisher", "humdrum:PPR", true),
    ("dateFirstPublished", "humdrum:PDT", false),
    ("publicationTitle", "humdrum:PTL", false),
    ("placeFirstPublished", "humdrum:PPP", false),
    ("publishersCatalogNumber", "humdrum:PC#", false),
    ("scholarlyCatalogName", "humdrum:SCA", false),
    ("scholarlyCatalogAbbreviation", "humdrum:SCT", false),
    ("manuscriptSourceName", "humdrum:SMS", false),
    ("manuscriptLocation", "humdrum:SML", false),
    ("manuscriptAccessAcknowledgement", "humdrum:SMA", false),
    ("originalDocumentOwner", "humdrum:YOO", true),
    ("originalEditor", "humdrum:YOE", true),
    ("electronicEditor", "humdrum:EED", true),
    ("electronicEncoder", "humdrum:ENC", true),
    ("electronicPublisher", "humdrum:YEP", true),
    ("electronicReleaseDate", "humdrum:YER", false),
    ("fileFormat", "m21FileInfo:fileFormat", false),
    ("filePath", "m21FileInfo:filePath", false),
    ("corpusFilePath", "m21FileInfo:corpusFilePath", false),
    ("fileNumber", "m21FileInfo:fileNumber", false),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_keep_the_order_they_were_first_given() {
        let mut md = Metadata::new();
        md.add_text("software", "Finale");
        md.add_text("movementName", "Grave");
        md.add_text("software", "Dolet");
        let all: Vec<(&str, &str)> = md.all().map(|(name, value)| (name, value.text())).collect();
        assert_eq!(
            all,
            [
                ("software", "Finale"),
                ("software", "Dolet"),
                ("movementName", "Grave")
            ]
        );
    }

    #[test]
    fn a_role_outside_the_vocabulary_is_another_contributor() {
        let mut md = Metadata::new();
        md.add_contributor("composer", "Caccini");
        md.add_contributor("tuba player", "Somebody");
        let names: Vec<&str> = md.contributors().map(|(name, _)| name).collect();
        assert_eq!(names, ["composer", "otherContributor"]);
        assert_eq!(md.get("otherContributor")[0].role(), Some("tuba player"));
    }

    #[test]
    fn a_search_of_one_name_reads_a_title_with_its_article_in_front() {
        let mut md = Metadata::new();
        md.add_text("title", "Grassy Turf, The");
        // music21 normalizes the article only where the field is named
        // outright, and leaves the stored text alone everywhere else.
        let query = MetadataQuery::Text("The Grassy");
        assert_eq!(
            md.search(&query, Some("title"))
                .map(|found| found.text().to_string()),
            Some("The Grassy Turf".to_string())
        );
        assert!(md.search(&query, None).is_none());
        assert!(md.search(&query, Some("titl")).is_none());
    }

    #[test]
    fn a_contributor_answers_under_the_role_it_was_given() {
        let mut md = Metadata::new();
        md.add_contributor("tuba player", "Somebody");
        let query = MetadataQuery::Text("somebody");
        assert_eq!(
            md.search(&query, None).and_then(|found| found.field()),
            Some("tuba player")
        );
        assert_eq!(
            md.search(&query, Some("tuba"))
                .and_then(|found| found.field()),
            Some("tuba player")
        );
        // A role of its own is not reached under `contributor`, as in
        // music21, which only asks that of a contributor with no role.
        assert!(md.search(&query, Some("contributor")).is_none());
    }

    #[test]
    fn a_matcher_searches_where_music21_would_read_a_pattern() {
        let mut md = Metadata::new();
        md.add_text("title", "Maple Leaf Rag");
        md.add_contributor("composer", "Joplin, Scott");
        let ends_in_rag = |text: &str| text.ends_with("Rag");
        let found = md
            .search(&MetadataQuery::Matcher(&ends_in_rag), None)
            .expect("the title");
        assert_eq!(
            (found.field(), found.text()),
            (Some("title"), "Maple Leaf Rag")
        );
        assert!(
            md.search(&MetadataQuery::Matcher(&ends_in_rag), Some("composer"))
                .is_none()
        );
    }

    #[test]
    fn the_vocabulary_names_its_namespaces() {
        assert_eq!(namespace_name("localeOfComposition"), Some("humdrum:OPC"));
        assert_eq!(namespace_name("dateCreated"), Some("dcterms:created"));
        assert_eq!(namespace_name("composer"), Some("marcrel:CMP"));
        assert_eq!(namespace_name("madeUp"), None);
        assert!(is_contributor_unique_name("lyricist"));
        assert!(!is_contributor_unique_name("title"));
    }
}
