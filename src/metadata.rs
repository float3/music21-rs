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

    /// Whether nothing is in it.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

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
    fn the_vocabulary_names_its_namespaces() {
        assert_eq!(namespace_name("localeOfComposition"), Some("humdrum:OPC"));
        assert_eq!(namespace_name("dateCreated"), Some("dcterms:created"));
        assert_eq!(namespace_name("composer"), Some("marcrel:CMP"));
        assert_eq!(namespace_name("madeUp"), None);
        assert!(is_contributor_unique_name("lyricist"));
        assert!(!is_contributor_unique_name("title"));
    }
}
