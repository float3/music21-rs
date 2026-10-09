//! Features of many pieces in one table, each row labelled with its piece's
//! class, written for machine learning: music21's `DataSet` and its output
//! formats, and the ways music21 finds extractors by id and by name.

use super::{DataInstance, Extractor, Feature, Value, jsymbolic, native};
use crate::{
    error::{Error, Result},
    metadata::Metadata,
    stream::Stream,
};

/// Where an extractor comes from: music21's jSymbolic ports or its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Library {
    /// [`jsymbolic::JSYMBOLIC`].
    JSymbolic,
    /// [`native::NATIVE`].
    Native,
}

impl Library {
    /// Both, jSymbolic first, as music21 looks through them.
    pub const ALL: [Library; 2] = [Library::JSymbolic, Library::Native];

    /// The library's extractors, in music21's order.
    pub fn extractors(self) -> &'static [Extractor] {
        match self {
            Self::JSymbolic => jsymbolic::JSYMBOLIC,
            Self::Native => native::NATIVE,
        }
    }

    /// The library's name as music21's `getIndex` reports it.
    pub fn name(self) -> &'static str {
        match self {
            Self::JSymbolic => "jsymbolic",
            Self::Native => "native",
        }
    }
}

/// The extractors with any of these ids, from these libraries, in the
/// libraries' own order: music21's `extractorsById`. An id is read without
/// regard to case, hyphens or spaces, so `p-20` finds `P20`; the id `all`,
/// first, finds every extractor. Both libraries have a `P22`, so asking for
/// it in both finds two.
pub fn extractors_by_id(ids: &[&str], libraries: &[Library]) -> Vec<&'static Extractor> {
    let wanted: Vec<String> = ids
        .iter()
        .map(|id| id.trim().to_lowercase().replace(['-', ' '], ""))
        .collect();
    let Some(first) = wanted.first() else {
        return Vec::new();
    };
    let everything = first == "all";
    libraries
        .iter()
        .flat_map(|library| library.extractors())
        .filter(|extractor| everything || wanted.contains(&extractor.id().to_lowercase()))
        .collect()
}

/// The first extractor [`extractors_by_id`] finds for an id: music21's
/// `extractorById`.
pub fn extractor_by_id(id: &str, libraries: &[Library]) -> Option<&'static Extractor> {
    extractors_by_id(&[id], libraries).into_iter().next()
}

/// The values of the first extractor with an id, read from a piece:
/// music21's `vectorById`. Nothing for an id no extractor has.
///
/// # Errors
///
/// A piece that cannot be prepared, or the feature cannot be read from.
pub fn vector_by_id(stream: &Stream, id: &str) -> Result<Option<Vec<Value>>> {
    let Some(extractor) = extractor_by_id(id, &Library::ALL) else {
        return Ok(None);
    };
    let data = DataInstance::new(stream)?;
    Ok(Some(extractor.extract(&data)?.values().to_vec()))
}

/// Where the extractor of a feature named so sits in its library, and
/// which library, looking through jSymbolic first unless one is given:
/// music21's `getIndex`, as `(61, JSymbolic)` for `Range`. Nothing for a
/// name no extractor has.
///
/// music21's own native list ends with its language feature, which is not
/// here, so its index is never answered.
pub fn index_of(name: &str, library: Option<Library>) -> Option<(usize, Library)> {
    let libraries: &[Library] = match library {
        Some(Library::JSymbolic) => &[Library::JSymbolic],
        Some(Library::Native) => &[Library::Native],
        None => &Library::ALL,
    };
    libraries.iter().find_map(|library| {
        library
            .extractors()
            .iter()
            .position(|extractor| extractor.name() == name)
            .map(|index| (index, *library))
    })
}

/// Every feature of a piece, jSymbolic's and then music21's own, each a
/// blank one where it cannot be read: music21's `allFeaturesAsList`, but
/// for its language feature.
///
/// # Errors
///
/// A piece that cannot be prepared.
pub fn all_features_as_list(stream: &Stream) -> Result<Vec<Vec<Value>>> {
    let mut set = DataSet::new("");
    set.add_extractors(Library::ALL.iter().flat_map(|library| library.extractors()));
    set.add_data(stream, "", None)?;
    set.process();
    Ok(set.features()[0]
        .iter()
        .map(|feature| feature.values().to_vec())
        .collect())
}

/// A file format a [`DataSet`] is written in: music21's `OutputFormat`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum OutputFormat {
    /// Tab-separated, with the two header rows Orange reads: music21's
    /// `OutputTabOrange`.
    Tab,
    /// Comma-separated values: music21's `OutputCSV`.
    Csv,
    /// Weka's Attribute-Relation File Format: music21's `OutputARFF`.
    Arff,
}

impl OutputFormat {
    /// The format a name stands for, as music21 reads one: `tab`, `orange`
    /// or `taborange`; `csv` or `comma`; `arff` or `attribute`; in any case.
    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "tab" | "orange" | "taborange" => Some(Self::Tab),
            "csv" | "comma" => Some(Self::Csv),
            "arff" | "attribute" => Some(Self::Arff),
            _ => None,
        }
    }

    /// The format a file's extension names, `data.csv` for
    /// [`OutputFormat::Csv`].
    pub fn from_path(path: &str) -> Option<Self> {
        path.rsplit_once('.')
            .and_then(|(_, extension)| Self::from_name(extension))
    }

    /// The extension music21 gives a file in this format, dot and all.
    pub fn extension(self) -> &'static str {
        match self {
            Self::Tab => ".tab",
            Self::Csv => ".csv",
            Self::Arff => ".arff",
        }
    }
}

/// One cell of a [`DataSet`]'s table: a piece's id or class, or a value.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Cell {
    /// An id or a class value.
    Text(String),
    /// A feature's value.
    Value(Value),
}

/// Written as music21 writes the cell: the text, or the value as Python
/// writes the number.
impl std::fmt::Display for Cell {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Text(text) => f.write_str(text),
            Self::Value(value) => write!(f, "{value}"),
        }
    }
}

/// A piece added to a data set.
#[derive(Debug)]
struct Entry {
    data: DataInstance,
    id: String,
    class_value: String,
}

/// An extractor that could not read its feature from a piece, so that the
/// data set stands a blank feature in for it.
#[derive(Clone, Debug)]
pub struct Failure {
    /// Which piece, counted from nought in the order they were added.
    pub row: usize,
    /// The extractor's id.
    pub extractor: &'static str,
    /// Why.
    pub error: Error,
}

/// Features of many pieces in one table: music21's `DataSet`. Each piece
/// is a row, labelled with an id and a value of the class the set is about
/// -- its composer, say -- and each extractor gives it as many columns as
/// it has values. [`DataSet::process`] reads every feature of every piece,
/// and [`DataSet::to_text`] writes the table for a machine-learning tool.
///
/// ```
/// use music21_rs::features::{DataSet, Library, OutputFormat, extractors_by_id};
/// use music21_rs::tinynotation::from_tiny_notation;
///
/// let mut set = DataSet::new("Composer");
/// set.add_extractors(extractors_by_id(&["R31", "R35"], &[Library::JSymbolic]));
/// set.add_data(&from_tiny_notation("4/4 c4 d e2")?, "Bach", Some("first piece"))?;
/// set.process();
/// assert_eq!(
///     set.to_text(OutputFormat::Csv),
///     "Identifier,Initial_Time_Signature_0,Initial_Time_Signature_1,Changes_of_Meter,Composer\n\
///      first_piece,4,4,0,Bach"
/// );
/// # Ok::<(), music21_rs::Error>(())
/// ```
#[derive(Debug)]
pub struct DataSet {
    class_label: String,
    extractors: Vec<&'static Extractor>,
    entries: Vec<Entry>,
    features: Vec<Vec<Feature>>,
    failures: Vec<Failure>,
}

impl DataSet {
    /// An empty data set about a class, named as its column is: music21's
    /// `DataSet(classLabel=...)`.
    pub fn new(class_label: impl Into<String>) -> Self {
        Self {
            class_label: class_label.into(),
            extractors: Vec::new(),
            entries: Vec::new(),
            features: Vec::new(),
            failures: Vec::new(),
        }
    }

    /// The name of the class the set is about.
    pub fn class_label(&self) -> &str {
        &self.class_label
    }

    /// The extractors, in the order their columns come.
    pub fn extractors(&self) -> &[&'static Extractor] {
        &self.extractors
    }

    /// Adds extractors after those already there: music21's
    /// `addFeatureExtractors`.
    pub fn add_extractors(&mut self, extractors: impl IntoIterator<Item = &'static Extractor>) {
        self.extractors.extend(extractors);
    }

    /// Adds a piece with its value of the class, prepared at once: music21's
    /// `addData`. Its id is the one given or, without one, the title of its
    /// metadata, or nothing; spaces in it become underscores, as music21's
    /// `getId` writes them.
    ///
    /// # Errors
    ///
    /// A piece that cannot be prepared.
    pub fn add_data(
        &mut self,
        stream: &Stream,
        class_value: impl Into<String>,
        id: Option<&str>,
    ) -> Result<()> {
        let id = match id {
            Some(id) => id.to_string(),
            None => stream
                .metadata()
                .and_then(Metadata::title)
                .unwrap_or_default(),
        };
        self.entries.push(Entry {
            data: DataInstance::new(stream)?,
            id: id.replace(' ', "_"),
            class_value: class_value.into(),
        });
        Ok(())
    }

    /// How many pieces have been added.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether no piece has been added.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Reads every feature of every piece, standing a blank feature in for
    /// one that cannot be read and keeping why in [`Self::failures`]:
    /// music21's `process`.
    pub fn process(&mut self) {
        self.features.clear();
        self.failures.clear();
        for (row, entry) in self.entries.iter().enumerate() {
            let mut features = Vec::with_capacity(self.extractors.len());
            for extractor in &self.extractors {
                features.push(match extractor.extract(&entry.data) {
                    Ok(feature) => feature,
                    Err(error) => {
                        self.failures.push(Failure {
                            row,
                            extractor: extractor.id(),
                            error,
                        });
                        extractor.blank()
                    }
                });
            }
            self.features.push(features);
        }
    }

    /// Reads every feature of every piece, stopping at the first that cannot
    /// be read: music21's `process` with `failFast`.
    ///
    /// # Errors
    ///
    /// The first feature that cannot be read.
    pub fn try_process(&mut self) -> Result<()> {
        self.features.clear();
        self.failures.clear();
        for entry in &self.entries {
            let features = self
                .extractors
                .iter()
                .map(|extractor| extractor.extract(&entry.data))
                .collect::<Result<Vec<_>>>()?;
            self.features.push(features);
        }
        Ok(())
    }

    /// The features read, a row for each piece and a feature for each
    /// extractor; empty until processed.
    pub fn features(&self) -> &[Vec<Feature>] {
        &self.features
    }

    /// The features [`Self::process`] could not read.
    pub fn failures(&self) -> &[Failure] {
        &self.failures
    }

    /// The column names: the id's, each extractor's attribute labels, and
    /// the class's, spaces made underscores: music21's `getAttributeLabels`.
    pub fn attribute_labels(&self, include_class_label: bool, include_id: bool) -> Vec<String> {
        let mut labels = Vec::new();
        if include_id {
            labels.push("Identifier".to_string());
        }
        for extractor in &self.extractors {
            labels.extend(extractor.attribute_labels());
        }
        if include_class_label {
            labels.push(self.class_label.replace(' ', "_"));
        }
        labels
    }

    /// Whether each column is discrete, nothing for the id's and the class
    /// always discrete: music21's `getDiscreteLabels`.
    pub fn discrete_labels(
        &self,
        include_class_label: bool,
        include_id: bool,
    ) -> Vec<Option<bool>> {
        let mut labels = Vec::new();
        if include_id {
            labels.push(None);
        }
        for extractor in &self.extractors {
            labels.extend(std::iter::repeat_n(
                Some(extractor.discrete()),
                extractor.dimensions(),
            ));
        }
        if include_class_label {
            labels.push(Some(true));
        }
        labels
    }

    /// Whether each column is the class's, nothing for the id's: music21's
    /// `getClassPositionLabels`.
    pub fn class_position_labels(
        &self,
        include_id: bool,
        include_class_label: bool,
    ) -> Vec<Option<bool>> {
        let mut labels = Vec::new();
        if include_id {
            labels.push(None);
        }
        for extractor in &self.extractors {
            labels.extend(std::iter::repeat_n(Some(false), extractor.dimensions()));
        }
        if include_class_label {
            labels.push(Some(true));
        }
        labels
    }

    /// The table, a row for each piece: its id, every value of every
    /// feature, and its class value: music21's `getFeaturesAsList`.
    pub fn features_as_list(&self, include_class_label: bool, include_id: bool) -> Vec<Vec<Cell>> {
        self.entries
            .iter()
            .zip(&self.features)
            .map(|(entry, features)| {
                let mut row = Vec::new();
                if include_id {
                    row.push(Cell::Text(entry.id.clone()));
                }
                for feature in features {
                    row.extend(feature.values().iter().copied().map(Cell::Value));
                }
                if include_class_label {
                    row.push(Cell::Text(entry.class_value.clone()));
                }
                row
            })
            .collect()
    }

    /// Each class value once, in the order first met: music21's
    /// `getUniqueClassValues`.
    pub fn unique_class_values(&self) -> Vec<&str> {
        let mut values: Vec<&str> = Vec::new();
        for entry in &self.entries {
            if !values.contains(&entry.class_value.as_str()) {
                values.push(&entry.class_value);
            }
        }
        values
    }

    /// The table written in a format, ids and classes included, lines
    /// joined with newlines and none after the last: music21's `getString`.
    pub fn to_text(&self, format: OutputFormat) -> String {
        let rows = self.features_as_list(true, true);
        let joined = |row: &[Cell], separator: &str| -> String {
            row.iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(separator)
        };
        let mut lines: Vec<String> = Vec::new();
        match format {
            OutputFormat::Tab => {
                lines.push(self.attribute_labels(true, true).join("\t"));
                lines.push(
                    self.discrete_labels(true, true)
                        .into_iter()
                        .map(|discrete| match discrete {
                            None => "string",
                            Some(true) => "discrete",
                            Some(false) => "continuous",
                        })
                        .collect::<Vec<_>>()
                        .join("\t"),
                );
                lines.push(
                    self.class_position_labels(true, true)
                        .into_iter()
                        .map(|class| match class {
                            None => "meta",
                            Some(true) => "class",
                            Some(false) => "",
                        })
                        .collect::<Vec<_>>()
                        .join("\t"),
                );
                lines.extend(rows.iter().map(|row| joined(row, "\t")));
            }
            OutputFormat::Csv => {
                lines.push(self.attribute_labels(true, true).join(","));
                lines.extend(rows.iter().map(|row| joined(row, ",")));
            }
            OutputFormat::Arff => {
                lines.push(format!("@RELATION {}", self.class_label));
                let labels = self.attribute_labels(true, true);
                let discrete = self.discrete_labels(true, true);
                let classes = self.class_position_labels(true, true);
                for ((label, discrete), class) in labels.iter().zip(discrete).zip(classes) {
                    lines.push(match (class, discrete) {
                        (Some(true), _) => {
                            format!(
                                "@ATTRIBUTE class {{{}}}",
                                self.unique_class_values().join(",")
                            )
                        }
                        (_, None) => format!("@ATTRIBUTE {label} STRING"),
                        _ => format!("@ATTRIBUTE {label} NUMERIC"),
                    });
                }
                lines.push("@DATA".to_string());
                lines.extend(rows.iter().map(|row| joined(row, ",")));
            }
        }
        lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tinynotation::from_tiny_notation;

    fn two_pieces() -> Result<DataSet> {
        let line = from_tiny_notation("4/4 c4 d e2")?;
        let mut set = DataSet::new("Composer");
        set.add_extractors(extractors_by_id(&["R31"], &[Library::JSymbolic]));
        set.add_extractors(extractors_by_id(&["P22"], &[Library::Native]));
        set.add_extractors(extractors_by_id(&["R35"], &[Library::JSymbolic]));
        set.add_data(&line, "Bach", Some("my piece"))?;
        set.add_data(&line, "Handel", None)?;
        set.process();
        Ok(set)
    }

    #[test]
    fn a_data_set_is_written_as_music21_writes_one() -> Result<()> {
        // Each read off music21's DataSet.getString for the same set.
        let set = two_pieces()?;
        assert_eq!(
            set.to_text(OutputFormat::Tab),
            "Identifier\tInitial_Time_Signature_0\tInitial_Time_Signature_1\tQuality\t\
             Changes_of_Meter\tComposer\nstring\tdiscrete\tdiscrete\tdiscrete\tdiscrete\t\
             discrete\nmeta\t\t\t\t\tclass\nmy_piece\t4\t4\t1\t0\tBach\n\t4\t4\t1\t0\tHandel"
        );
        assert_eq!(
            set.to_text(OutputFormat::Csv),
            "Identifier,Initial_Time_Signature_0,Initial_Time_Signature_1,Quality,\
             Changes_of_Meter,Composer\nmy_piece,4,4,1,0,Bach\n,4,4,1,0,Handel"
        );
        assert_eq!(
            set.to_text(OutputFormat::Arff),
            "@RELATION Composer\n@ATTRIBUTE Identifier STRING\n\
             @ATTRIBUTE Initial_Time_Signature_0 NUMERIC\n\
             @ATTRIBUTE Initial_Time_Signature_1 NUMERIC\n@ATTRIBUTE Quality NUMERIC\n\
             @ATTRIBUTE Changes_of_Meter NUMERIC\n@ATTRIBUTE class {Bach,Handel}\n@DATA\n\
             my_piece,4,4,1,0,Bach\n,4,4,1,0,Handel"
        );
        Ok(())
    }

    #[test]
    fn extractors_are_found_by_id_and_by_name() {
        let ids = |found: Vec<&'static Extractor>| -> Vec<&str> {
            found.iter().map(|extractor| extractor.id()).collect()
        };
        assert_eq!(
            ids(extractors_by_id(&["p22", "ql1", "P-20"], &Library::ALL)),
            ["P20", "P22", "P22", "QL1"]
        );
        assert_eq!(ids(extractors_by_id(&["p 21"], &Library::ALL)), ["P21"]);
        let all = extractors_by_id(&["all"], &Library::ALL);
        assert_eq!(all.len(), 91);
        assert_eq!(index_of("Range", None), Some((61, Library::JSymbolic)));
        assert_eq!(
            index_of("Ends With Landini Melodic Contour", None),
            Some((18, Library::Native))
        );
        assert_eq!(
            index_of("Tonal Certainty", Some(Library::Native)),
            Some((1, Library::Native))
        );
        assert_eq!(index_of("aBrandNewFeature!", None), None);
        assert_eq!(OutputFormat::from_path("test.tab"), Some(OutputFormat::Tab));
        assert_eq!(OutputFormat::from_path("junk"), None);
    }

    #[test]
    fn every_feature_of_a_piece_is_listed() -> Result<()> {
        // music21: features.allFeaturesAsList(s)[2:5] == [[2], [2], [1.0]]
        let features = all_features_as_list(&from_tiny_notation("4/4 c4 d e2")?)?;
        assert_eq!(features.len(), 91);
        assert_eq!(
            features[2..5],
            [
                vec![Value::Integer(2)],
                vec![Value::Integer(2)],
                vec![Value::Float(1.0)]
            ]
        );
        Ok(())
    }
}
