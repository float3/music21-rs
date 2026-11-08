//! music21's user environment: the preferences kept in `.music21rc`, where
//! that file lives, and where temporary files go.
//!
//! music21 keeps a fixed set of preferences: where MuseScore, Lilypond and
//! the viewers are, which format `show()` and `write()` use, whether the
//! corpus may download, how much debugging to print. Each has a default
//! that depends on the platform; the user's own values are read from an XML
//! file and written back to it in music21's layout, so a file written here
//! reads in music21 and the other way round.
//!
//! ```
//! use music21::environment::{self, Platform};
//!
//! let defaults = environment::defaults(Platform::Nix, (0, 0, 0));
//! let musicxml = defaults.iter().find(|(key, _)| *key == "musicxmlPath");
//! assert_eq!(
//!     musicxml.map(|(_, value)| value),
//!     Some(&environment::Value::text("/usr/bin/mscore3"))
//! );
//! assert_eq!(environment::format_to_key("png"), Some("graphicsPath"));
//! ```

use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

/// The three kinds of system music21 tells apart, by the names it gives
/// them: `'win'`, `'darwin'` and `'nix'`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Platform {
    /// Windows: music21's `'win'`.
    Windows,
    /// macOS: music21's `'darwin'`.
    Darwin,
    /// Anything else, Linux among them: music21's `'nix'`.
    Nix,
}

impl Platform {
    /// The platform this is running on, as music21's `getPlatform` says.
    ///
    /// ```
    /// use music21::environment::Platform;
    ///
    /// assert_eq!(Platform::from_name(Platform::current().as_str()), Some(Platform::current()));
    /// ```
    pub fn current() -> Self {
        match std::env::consts::OS {
            "windows" => Self::Windows,
            "macos" => Self::Darwin,
            _ => Self::Nix,
        }
    }

    /// music21's name for it.
    ///
    /// ```
    /// assert_eq!(music21::environment::Platform::Darwin.as_str(), "darwin");
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Windows => "win",
            Self::Darwin => "darwin",
            Self::Nix => "nix",
        }
    }

    /// The platform music21 names so, if any.
    ///
    /// ```
    /// use music21::environment::Platform;
    ///
    /// assert_eq!(Platform::from_name("win"), Some(Platform::Windows));
    /// assert_eq!(Platform::from_name("amiga"), None);
    /// ```
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "win" => Some(Self::Windows),
            "darwin" => Some(Self::Darwin),
            "nix" => Some(Self::Nix),
            _ => None,
        }
    }
}

/// music21's `common.DEBUG_OFF`: no debugging output.
pub const DEBUG_OFF: i64 = 0;
/// music21's `common.DEBUG_USER`: what a user would want to see.
pub const DEBUG_USER: i64 = 1;
/// music21's `common.DEBUG_DEVEL`: what a developer would.
pub const DEBUG_DEVEL: i64 = 63;
/// music21's `common.DEBUG_ALL`: everything.
pub const DEBUG_ALL: i64 = 255;

/// The preferences that name a file or an application, in music21's order.
pub const PATH_KEYS: [&str; 10] = [
    "braillePath",
    "graphicsPath",
    "lilypondPath",
    "localCorpusPath",
    "manualCoreCorpusPath",
    "midiPath",
    "musescoreDirectPNGPath",
    "musicxmlPath",
    "pdfPath",
    "vectorPath",
];

/// What `showFormat` may be: music21's `common.VALID_SHOW_FORMATS`.
pub const SHOW_FORMATS: [&str; 17] = [
    "musicxml",
    "lilypond",
    "text",
    "textline",
    "midi",
    "png",
    "pdf",
    "svg",
    "lily.pdf",
    "lily.png",
    "lily.svg",
    "braille",
    "ipython",
    "ipython.png",
    "ipython.midi",
    "musicxml.png",
    "musicxml.pdf",
];

/// What `writeFormat` may be: music21's `common.VALID_WRITE_FORMATS`.
pub const WRITE_FORMATS: [&str; 18] = [
    "musicxml",
    "lilypond",
    "text",
    "textline",
    "midi",
    "png",
    "pdf",
    "svg",
    "lily.pdf",
    "lily.png",
    "lily.svg",
    "braille",
    "ipython",
    "ipython.png",
    "ipython.midi",
    "musicxml.png",
    "musicxml.pdf",
    "rntxt",
];

/// What `autoDownload` may be: music21's `common.VALID_AUTO_DOWNLOAD`.
pub const AUTO_DOWNLOAD: [&str; 3] = ["ask", "deny", "allow"];

/// A local corpus's settings: the directories it reads, the file its
/// metadata is cached in, and its name, which the unnamed corpus has none
/// of. music21's `LocalCorpusSettings`.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LocalCorpus {
    /// Its name; `None` for the unnamed local corpus.
    pub name: Option<String>,
    /// The directories it reads.
    pub paths: Vec<String>,
    /// Where its metadata is cached; `None` for the temporary directory.
    pub cache_file_path: Option<String>,
}

/// The value of one preference.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Value {
    /// Not set: music21's `None`.
    Unset,
    /// Text, as every preference read back from a file is.
    Text(String),
    /// A number: `debug` and `warnings` before they are written out.
    Number(i64),
    /// `localCorpusSettings`: the unnamed local corpus.
    LocalCorpus(LocalCorpus),
    /// `localCorporaSettings`: the named local corpora, by name.
    LocalCorpora(Vec<LocalCorpus>),
}

impl Value {
    /// A text value.
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text(text.into())
    }
}

/// Every preference with its default on `platform`, in music21's order.
///
/// `mac_version` is the running macOS's version, which says where Preview
/// is; music21 reads it as `(0, 0, 0)` off a Mac. The paths come as music21
/// stores them, cleaned as [`clean_path`] cleans them.
///
/// ```
/// use music21::environment::{self, Platform, Value};
///
/// let defaults = environment::defaults(Platform::Darwin, (14, 0, 0));
/// let preview = defaults.iter().find(|(key, _)| *key == "pdfPath");
/// assert_eq!(
///     preview.map(|(_, value)| value),
///     Some(&Value::text("/System/Applications/Preview.app"))
/// );
/// assert_eq!(defaults.len(), 21);
/// ```
pub fn defaults(platform: Platform, mac_version: (u32, u32, u32)) -> Vec<(&'static str, Value)> {
    let mut found = vec![
        ("directoryScratch", Value::Unset),
        ("lilypondPath", Value::Unset),
        ("lilypondVersion", Value::Unset),
        ("lilypondFormat", Value::text("pdf")),
        ("lilypondBackend", Value::text("ps")),
        ("musicxmlPath", Value::Unset),
        ("midiPath", Value::Unset),
        ("graphicsPath", Value::Unset),
        ("vectorPath", Value::Unset),
        ("pdfPath", Value::Unset),
        ("braillePath", Value::Unset),
        ("musescoreDirectPNGPath", Value::Unset),
        ("showFormat", Value::text("musicxml")),
        ("writeFormat", Value::text("musicxml")),
        ("ipythonShowFormat", Value::text("ipython.musicxml.png")),
        ("autoDownload", Value::text("ask")),
        ("debug", Value::Number(DEBUG_OFF)),
        ("warnings", Value::Number(1)),
        (
            "localCorpusSettings",
            Value::LocalCorpus(LocalCorpus::default()),
        ),
        ("localCorporaSettings", Value::LocalCorpora(Vec::new())),
        ("manualCoreCorpusPath", Value::Unset),
    ];
    for (key, path) in platform_paths(platform, mac_version) {
        if let Some(entry) = found.iter_mut().find(|(name, _)| *name == key) {
            entry.1 = Value::Text(clean_path(&path));
        }
    }
    found
}

/// The applications music21 expects to find on each platform.
fn platform_paths(platform: Platform, mac_version: (u32, u32, u32)) -> Vec<(&'static str, String)> {
    match platform {
        Platform::Windows => {
            let musescore = clean_path(r"%PROGRAMFILES%\MuseScore 4\bin\MuseScore4.exe");
            vec![
                ("lilypondPath", "lilypond".to_owned()),
                ("musicxmlPath", musescore.clone()),
                ("musescoreDirectPNGPath", musescore),
            ]
        }
        Platform::Nix => vec![
            ("lilypondPath", "/usr/bin/lilypond".to_owned()),
            ("musicxmlPath", "/usr/bin/mscore3".to_owned()),
            ("musescoreDirectPNGPath", "/usr/bin/mscore3".to_owned()),
            ("graphicsPath", "/usr/bin/xdg-open".to_owned()),
            ("pdfPath", "/usr/bin/xdg-open".to_owned()),
        ],
        Platform::Darwin => {
            let (major, minor, _) = mac_version;
            let preview = if major >= 11 || (major == 10 && minor >= 15) {
                "/System/Applications/Preview.app"
            } else {
                "/Applications/Preview.app"
            };
            let musescore = "/Applications/MuseScore 4.app/Contents/MacOS/mscore";
            vec![
                (
                    "lilypondPath",
                    "/Applications/Lilypond.app/Contents/Resources/bin/lilypond".to_owned(),
                ),
                ("musicxmlPath", musescore.to_owned()),
                ("graphicsPath", preview.to_owned()),
                ("vectorPath", preview.to_owned()),
                ("pdfPath", preview.to_owned()),
                ("midiPath", "/Applications/GarageBand.app".to_owned()),
                ("musescoreDirectPNGPath", musescore.to_owned()),
            ]
        }
    }
}

/// Whether a preference names a file or an application.
///
/// ```
/// assert!(music21::environment::is_path_key("musicxmlPath"));
/// assert!(!music21::environment::is_path_key("debug"));
/// ```
pub fn is_path_key(key: &str) -> bool {
    PATH_KEYS.contains(&key)
}

/// `value` as `showFormat`, `writeFormat` or `autoDownload` takes it: in
/// lower case, and an error where it is not one music21 allows. Any other
/// preference takes any value, and `value` comes back as it was given.
///
/// ```
/// use music21::environment::accepted;
///
/// assert_eq!(accepted("showFormat", "MusicXML").unwrap(), "musicxml");
/// assert_eq!(
///     accepted("autoDownload", "asdf").unwrap_err().to_string(),
///     "Environment error: asdf is not an acceptable value for preference: autoDownload"
/// );
/// ```
pub fn accepted(key: &str, value: &str) -> Result<String> {
    let allowed: &[&str] = match key {
        "showFormat" => &SHOW_FORMATS,
        "writeFormat" => &WRITE_FORMATS,
        "autoDownload" => &AUTO_DOWNLOAD,
        _ => return Ok(value.to_owned()),
    };
    let lowered = value.to_lowercase();
    if allowed.contains(&lowered.as_str()) {
        return Ok(lowered);
    }
    Err(Error::Environment(format!(
        "{lowered} is not an acceptable value for preference: {key}"
    )))
}

/// The debugging level `debug` is set to, read as music21 reads it: `true`
/// and `false`, a word naming a level, or a number.
///
/// ```
/// use music21::environment::debug_level;
///
/// assert_eq!(debug_level("True").unwrap(), 1);
/// assert_eq!(debug_level("devel").unwrap(), 63);
/// assert_eq!(debug_level("7").unwrap(), 7);
/// assert!(debug_level("loud").is_err());
/// ```
pub fn debug_level(value: &str) -> Result<i64> {
    let lowered = value.to_lowercase();
    if lowered == "true" {
        return Ok(DEBUG_USER);
    }
    if lowered == "false" || lowered.contains("off") {
        return Ok(DEBUG_OFF);
    }
    if lowered.contains("user") {
        return Ok(DEBUG_USER);
    }
    if lowered.contains("devel") {
        return Ok(DEBUG_DEVEL);
    }
    if lowered.contains("all") {
        return Ok(DEBUG_ALL);
    }
    let digits: String = value.trim().chars().filter(|&c| c != '_').collect();
    digits
        .parse()
        .map_err(|_| Error::Value(format!("invalid literal for int() with base 10: {value:?}")))
}

/// The preference naming the application that opens a format, if any.
///
/// ```
/// use music21::environment::format_to_key;
///
/// assert_eq!(format_to_key("lilypond"), Some("lilypondPath"));
/// assert_eq!(format_to_key("jpeg"), Some("graphicsPath"));
/// assert_eq!(format_to_key("ipython"), None);
/// ```
pub fn format_to_key(format: &str) -> Option<&'static str> {
    match format {
        "lilypond" => Some("lilypondPath"),
        "png" | "jpeg" => Some("graphicsPath"),
        "svg" => Some("vectorPath"),
        "pdf" => Some("pdfPath"),
        "musicxml" => Some("musicxmlPath"),
        "midi" => Some("midiPath"),
        "braille" => Some("braillePath"),
        _ => None,
    }
}

/// Which MusicXML reader a path names: music21's `xmlReaderType`.
///
/// ```
/// use music21::environment::xml_reader_type;
///
/// assert_eq!(xml_reader_type("/Applications/Sibelius 7.app"), Some("Sibelius"));
/// assert_eq!(xml_reader_type("/usr/bin/mscore3"), Some("unknown"));
/// assert_eq!(xml_reader_type(""), None);
/// ```
pub fn xml_reader_type(path: &str) -> Option<&'static str> {
    if path.is_empty() {
        return None;
    }
    let lowered = path.to_lowercase();
    if lowered.contains("sibelius") {
        return Some("Sibelius");
    }
    if lowered.contains("finale") {
        return Some("Finale");
    }
    if lowered.contains("musescore") {
        return Some("Musescore");
    }
    Some("unknown")
}

/// A path as music21's `common.cleanpath` leaves it: `~` expanded, made
/// absolute against the working directory and normalized, and environment
/// variables expanded.
///
/// A `..` is collapsed only where the directories before it exist, as the
/// system would; otherwise the path is joined to the working directory as
/// written.
///
/// ```
/// use music21::environment::clean_path;
///
/// assert_eq!(clean_path("/usr//bin/./lilypond"), "/usr/bin/lilypond");
/// assert!(clean_path("relative/file.xml").ends_with("/relative/file.xml"));
/// ```
pub fn clean_path(path: &str) -> String {
    let expanded = expand_user(path);
    let parts: Vec<&str> = expanded.split('/').collect();
    let normalizable = match parts.iter().rposition(|part| *part == "..") {
        Some(last) => {
            let through = parts[..=last].join("/");
            let through = if through.is_empty() {
                "/".to_owned()
            } else {
                through
            };
            Path::new(&through).is_dir()
        }
        None => true,
    };
    let cleaned = if normalizable {
        absolute(&normalized(&expanded))
    } else {
        join(&working_directory(), &expanded)
    };
    expand_variables(&cleaned)
}

/// `~` and `~user` at the start expanded, as `os.path.expanduser` does on
/// a POSIX system: `~` from `HOME`.
fn expand_user(path: &str) -> String {
    if !path.starts_with('~') {
        return path.to_owned();
    }
    let end = path.find('/').unwrap_or(path.len());
    let home = if end == 1 {
        std::env::var("HOME").ok()
    } else {
        // Another user's home: where it would be beside this user's.
        std::env::var("HOME").ok().and_then(|home| {
            Path::new(&home)
                .parent()
                .map(|parent| parent.join(&path[1..end]).to_string_lossy().into_owned())
        })
    };
    let Some(home) = home else {
        return path.to_owned();
    };
    let home = home.trim_end_matches('/');
    let home = if home.is_empty() { "/" } else { home };
    let rest = &path[end..];
    if home == "/" && rest.starts_with('/') {
        return rest.to_owned();
    }
    format!("{home}{rest}")
}

/// A path with `.`, doubled slashes and `name/..` taken out, as
/// `os.path.normpath` takes them out on a POSIX system.
fn normalized(path: &str) -> String {
    if path.is_empty() {
        return ".".to_owned();
    }
    let leading = if path.starts_with("//") && !path.starts_with("///") {
        "//"
    } else if path.starts_with('/') {
        "/"
    } else {
        ""
    };
    let mut kept: Vec<&str> = Vec::new();
    for part in path.split('/') {
        if part.is_empty() || part == "." {
            continue;
        }
        let climbs = part == "..";
        if climbs && kept.last().is_some_and(|last| *last != "..") {
            kept.pop();
            continue;
        }
        if climbs && !leading.is_empty() {
            continue;
        }
        kept.push(part);
    }
    let joined = format!("{leading}{}", kept.join("/"));
    if joined.is_empty() {
        return ".".to_owned();
    }
    joined
}

/// The working directory, as text.
fn working_directory() -> String {
    std::env::current_dir()
        .map(|directory| directory.to_string_lossy().into_owned())
        .unwrap_or_else(|_| "/".to_owned())
}

/// `os.path.join` of two paths: the second alone where it is absolute.
fn join(first: &str, second: &str) -> String {
    if second.starts_with('/') || first.is_empty() {
        return second.to_owned();
    }
    if first.ends_with('/') {
        return format!("{first}{second}");
    }
    format!("{first}/{second}")
}

/// `os.path.abspath` of a path already normalized.
fn absolute(path: &str) -> String {
    if path.starts_with('/') {
        return path.to_owned();
    }
    normalized(&join(&working_directory(), path))
}

/// `$name` and `${name}` replaced by the variable's value, as
/// `os.path.expandvars` does on a POSIX system; one that is not set is
/// left as written.
fn expand_variables(path: &str) -> String {
    if !path.contains('$') {
        return path.to_owned();
    }
    let is_name = |c: char| c.is_ascii_alphanumeric() || c == '_';
    let mut expanded = String::new();
    let mut rest = path;
    while let Some(dollar) = rest.find('$') {
        expanded.push_str(&rest[..dollar]);
        let after = &rest[dollar + 1..];
        let (name, written_length) = if let Some(braced) = after.strip_prefix('{') {
            match braced.find('}') {
                Some(close) => (&braced[..close], close + 2),
                None => ("", 0),
            }
        } else {
            let length = after.find(|c: char| !is_name(c)).unwrap_or(after.len());
            (&after[..length], length)
        };
        let value = if name.is_empty() {
            None
        } else {
            std::env::var(name).ok()
        };
        match value {
            Some(value) => expanded.push_str(&value),
            None => expanded.push_str(&rest[dollar..=dollar + written_length]),
        }
        rest = &after[written_length..];
    }
    expanded.push_str(rest);
    expanded
}

/// The settings as music21 writes them out, sorted by name: a
/// `<settings>` element holding a `<preference>` for each value and an
/// element apiece for the local corpora.
///
/// Compact, as ElementTree writes a tree it has not indented; with
/// `indented`, as music21 writes `.music21rc`: two spaces a level and a
/// closing newline, and no XML declaration, which ElementTree leaves out
/// of a UTF-8 file.
///
/// ```
/// use music21::environment::{settings_xml, Value};
///
/// let settings = [("debug".to_owned(), Value::Number(0)), ("braillePath".to_owned(), Value::Unset)];
/// assert_eq!(
///     settings_xml(&settings, false),
///     "<settings encoding=\"utf-8\"><preference name=\"braillePath\" />\
///      <preference name=\"debug\" value=\"0\" /></settings>"
/// );
/// ```
pub fn settings_xml(settings: &[(String, Value)], indented: bool) -> String {
    let mut sorted: Vec<&(String, Value)> = settings.iter().collect();
    sorted.sort_by(|first, second| first.0.cmp(&second.0));
    let mut children = Vec::new();
    for (key, value) in sorted {
        children.push(setting_element(key, value));
    }
    let root = Element {
        tag: "settings",
        attributes: vec![("encoding", "utf-8".to_owned())],
        text: None,
        children,
    };
    let mut written = String::new();
    root.write(&mut written, indented.then_some(0));
    if indented {
        written.push('\n');
    }
    written
}

/// One element of a settings file.
struct Element {
    tag: &'static str,
    attributes: Vec<(&'static str, String)>,
    text: Option<String>,
    children: Vec<Element>,
}

impl Element {
    fn bare(tag: &'static str) -> Self {
        Self {
            tag,
            attributes: Vec::new(),
            text: None,
            children: Vec::new(),
        }
    }

    /// Written as ElementTree writes it, indented at `level` as music21's
    /// `etIndent` lays it out, or compact where `level` is `None`.
    fn write(&self, out: &mut String, level: Option<usize>) {
        out.push('<');
        out.push_str(self.tag);
        for (name, value) in &self.attributes {
            out.push_str(&format!(" {name}=\"{}\"", escaped_attribute(value)));
        }
        if self.children.is_empty() && self.text.as_deref().is_none_or(str::is_empty) {
            out.push_str(" />");
            return;
        }
        out.push('>');
        if let Some(text) = &self.text {
            out.push_str(&escaped_text(text));
        }
        for child in &self.children {
            if let Some(level) = level {
                out.push('\n');
                out.push_str(&"  ".repeat(level + 1));
            }
            child.write(out, level.map(|level| level + 1));
        }
        if let Some(level) = level
            && !self.children.is_empty()
        {
            out.push('\n');
            out.push_str(&"  ".repeat(level));
        }
        out.push_str("</");
        out.push_str(self.tag);
        out.push('>');
    }
}

/// The element one setting is written as.
fn setting_element(key: &str, value: &Value) -> Element {
    match value {
        Value::LocalCorpus(corpus) => corpus_element(corpus, false),
        Value::LocalCorpora(corpora) => {
            let mut sorted: Vec<&LocalCorpus> = corpora.iter().collect();
            sorted.sort_by(|first, second| first.name.cmp(&second.name));
            let mut element = Element::bare("localCorporaSettings");
            element.children = sorted
                .into_iter()
                .map(|corpus| corpus_element(corpus, true))
                .collect();
            element
        }
        Value::Unset => Element {
            attributes: vec![("name", key.to_owned())],
            ..Element::bare("preference")
        },
        Value::Text(text) => Element {
            attributes: vec![("name", key.to_owned()), ("value", text.clone())],
            ..Element::bare("preference")
        },
        Value::Number(number) => Element {
            attributes: vec![("name", key.to_owned()), ("value", number.to_string())],
            ..Element::bare("preference")
        },
    }
}

/// A local corpus's element: its paths, sorted, then its cache file.
fn corpus_element(corpus: &LocalCorpus, named: bool) -> Element {
    let mut element = Element::bare("localCorpusSettings");
    if named {
        element
            .attributes
            .push(("name", corpus.name.clone().unwrap_or_default()));
    }
    let mut paths: Vec<&String> = corpus.paths.iter().collect();
    paths.sort();
    for path in paths {
        element.children.push(Element {
            text: Some(path.clone()),
            ..Element::bare("localCorpusPath")
        });
    }
    if let Some(cache) = &corpus.cache_file_path {
        element.children.push(Element {
            text: Some(cache.clone()),
            ..Element::bare("cacheFilePath")
        });
    }
    element
}

/// Text escaped as ElementTree escapes an element's text.
fn escaped_text(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Text escaped as ElementTree escapes an attribute's value.
fn escaped_attribute(text: &str) -> String {
    escaped_text(text)
        .replace('"', "&quot;")
        .replace('\r', "&#13;")
        .replace('\n', "&#10;")
        .replace('\t', "&#09;")
}

/// The settings a settings file holds, in the order it holds them, as
/// music21's `_fromSettings` reads them: each `<preference>` by its name,
/// with its value or none, and the local corpora.
///
/// ```
/// use music21::environment::{read_settings, LocalCorpus, Value};
///
/// let read = read_settings(
///     "<settings><preference name=\"midiPath\" value=\"w\"/>\
///      <localCorpusSettings><localCorpusPath> x </localCorpusPath></localCorpusSettings>\
///      </settings>",
/// )
/// .unwrap();
/// assert_eq!(read[0], ("midiPath".to_owned(), Value::text("w")));
/// assert_eq!(
///     read[1].1,
///     Value::LocalCorpus(LocalCorpus { paths: vec!["x".to_owned()], ..Default::default() })
/// );
/// ```
pub fn read_settings(document: &str) -> Result<Vec<(String, Value)>> {
    let root =
        crate::xml::Xml::parse(document).map_err(|error| Error::Environment(error.to_string()))?;
    let mut found = Vec::new();
    for slot in &root.children {
        match slot.tag.as_str() {
            "localCorpusSettings" => {
                found.push((slot.tag.clone(), Value::LocalCorpus(read_corpus(slot))));
            }
            "localCorporaSettings" => {
                let corpora = slot.children.iter().map(read_corpus).collect();
                found.push((slot.tag.clone(), Value::LocalCorpora(corpora)));
            }
            _ => {
                let Some(name) = slot.get("name") else {
                    continue;
                };
                let value = match slot.get("value") {
                    Some(value) => Value::text(value),
                    None => Value::Unset,
                };
                found.push((name.to_owned(), value));
            }
        }
    }
    Ok(found)
}

/// One `<localCorpusSettings>` read back.
fn read_corpus(slot: &crate::xml::Xml) -> LocalCorpus {
    let mut corpus = LocalCorpus {
        name: slot.get("name").map(str::to_owned),
        ..LocalCorpus::default()
    };
    for child in &slot.children {
        match child.tag.as_str() {
            "localCorpusPath" => corpus.paths.push(child.stripped().to_owned()),
            "cacheFilePath" => corpus.cache_file_path = Some(child.stripped().to_owned()),
            _ => {}
        }
    }
    corpus
}

/// Where the settings file is on `platform`, as music21's
/// `getSettingsPath` finds it.
///
/// `variable` reads the environment, `writable` says whether a directory
/// can be written to, and `temporary` is the directory to fall back on
/// where the home directory cannot be: music21's
/// `getDefaultRootTempDir`.
///
/// ```
/// use std::path::{Path, PathBuf};
/// use music21::environment::{settings_path, Platform};
///
/// let home = |name: &str| (name == "HOME").then(|| "/home/me".to_owned());
/// let found = settings_path(Platform::Nix, home, |_: &Path| true, || PathBuf::from("/tmp/music21"));
/// assert_eq!(found, PathBuf::from("/home/me/.music21rc"));
/// let found = settings_path(Platform::Nix, |_: &str| None, |_: &Path| true, || PathBuf::from("/tmp/music21"));
/// assert_eq!(found, PathBuf::from("/tmp/music21/.music21rc"));
/// ```
pub fn settings_path(
    platform: Platform,
    variable: impl Fn(&str) -> Option<String>,
    writable: impl Fn(&Path) -> bool,
    temporary: impl FnOnce() -> PathBuf,
) -> PathBuf {
    if platform == Platform::Windows {
        let directory = match (variable("APPDATA"), variable("USERPROFILE")) {
            (Some(appdata), _) => PathBuf::from(appdata),
            (None, Some(profile)) => PathBuf::from(profile).join("Application Data"),
            (None, None) => PathBuf::from(expand_user("~")),
        };
        return directory.join("music21-settings.xml");
    }
    let directory = match variable("HOME") {
        Some(home) if writable(Path::new(&home)) => PathBuf::from(home),
        _ => temporary(),
    };
    directory.join(".music21rc")
}

/// Whether a path exists and can be read and written, roughly: it is there
/// and not marked read-only. Callers that can ask the system properly
/// (Python's `os.access`) pass their own answer to the functions here.
pub fn is_accessible(path: &Path) -> bool {
    std::fs::metadata(path).is_ok_and(|metadata| !metadata.permissions().readonly())
}

/// Whether a directory is safe to keep temporary files in: accessible and,
/// where `user` is given, a real directory (not a link) that user owns and
/// no one else can write to. music21's `_isSafeTempDir`.
pub fn is_safe_temp_dir(
    path: &Path,
    user: Option<u32>,
    accessible: impl Fn(&Path) -> bool,
) -> bool {
    if !accessible(path) {
        return false;
    }
    let Some(user) = user else {
        return true;
    };
    owned_privately(path, user)
}

#[cfg(unix)]
fn owned_privately(path: &Path, user: u32) -> bool {
    use std::os::unix::fs::MetadataExt;

    std::fs::symlink_metadata(path).is_ok_and(|metadata| {
        metadata.file_type().is_dir() && metadata.uid() == user && metadata.mode() & 0o022 == 0
    })
}

#[cfg(not(unix))]
fn owned_privately(path: &Path, _user: u32) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_dir())
}

/// A directory for music21's temporary files under `system_temp`, as
/// music21's `getDefaultRootTempDir` picks it: `music21` there, then
/// `music21-userid-<user>` where a user is given, each made if missing and
/// used only if [`is_safe_temp_dir`]; failing both, a new directory named
/// `music21-` and something random.
///
/// ```
/// use music21::environment::{default_root_temp_dir, is_accessible};
///
/// let system = std::env::temp_dir();
/// let found = default_root_temp_dir(&system, None, is_accessible).unwrap();
/// assert!(found.starts_with(&system));
/// ```
pub fn default_root_temp_dir(
    system_temp: &Path,
    user: Option<u32>,
    accessible: impl Fn(&Path) -> bool,
) -> std::io::Result<PathBuf> {
    let mut candidates = vec![system_temp.join("music21")];
    if let Some(user) = user {
        candidates.push(system_temp.join(format!("music21-userid-{user}")));
    }
    for candidate in candidates {
        // One that is there already, or cannot be made, is judged as it is.
        let _ = private_directory(&candidate);
        if is_safe_temp_dir(&candidate, user, &accessible) {
            return Ok(candidate);
        }
    }
    fresh_temp_dir(system_temp)
}

/// Makes a directory only its owner can open, as `mkdir(mode=0o700)` does.
fn private_directory(path: &Path) -> std::io::Result<()> {
    let mut builder = std::fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(path)
}

/// A new directory named `music21-` and eight random characters, as
/// `tempfile.mkdtemp(prefix='music21-')` makes one.
fn fresh_temp_dir(system_temp: &Path) -> std::io::Result<PathBuf> {
    use std::hash::{BuildHasher, Hasher};

    const LETTERS: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789_";
    let random = std::collections::hash_map::RandomState::new();
    for attempt in 0..100u64 {
        let mut hasher = random.build_hasher();
        hasher.write_u64(attempt);
        let mut seed = hasher.finish();
        let mut name = String::from("music21-");
        for _ in 0..8 {
            name.push(char::from(LETTERS[(seed % LETTERS.len() as u64) as usize]));
            seed /= LETTERS.len() as u64;
        }
        let candidate = system_temp.join(name);
        match private_directory(&candidate) {
            Ok(()) => return Ok(candidate),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "no unused temporary directory name was found",
    ))
}

/// A debugging or warning message as music21's `common.formatStr` makes
/// one: the parts joined by spaces, ending in a newline.
///
/// ```
/// assert_eq!(music21::environment::format_message(&["test", "1", "2"]), "test 1 2\n");
/// ```
pub fn format_message<S: AsRef<str>>(parts: &[S]) -> String {
    let joined: Vec<&str> = parts.iter().map(AsRef::as_ref).collect();
    format!("{}\n", joined.join(" "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn darwin_settings_are_written_as_music21_writes_them() {
        let settings: Vec<(String, Value)> = defaults(Platform::Darwin, (14, 0, 0))
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value))
            .collect();
        let written = settings_xml(&settings, true);
        let lines: Vec<&str> = written.lines().collect();
        assert_eq!(lines[0], "<settings encoding=\"utf-8\">");
        assert_eq!(
            lines[1],
            "  <preference name=\"autoDownload\" value=\"ask\" />"
        );
        assert_eq!(lines[11], "  <localCorporaSettings />");
        assert_eq!(lines[12], "  <localCorpusSettings />");
        assert_eq!(lines.last(), Some(&"</settings>"));
        assert!(written.ends_with("</settings>\n"));
    }

    #[test]
    fn local_corpora_are_written_nested_and_read_back() {
        let corpus = LocalCorpus {
            name: Some("foo".to_owned()),
            paths: vec!["baz".to_owned(), "bar".to_owned()],
            cache_file_path: Some("/tmp/local.json".to_owned()),
        };
        let settings = vec![(
            "localCorporaSettings".to_owned(),
            Value::LocalCorpora(vec![corpus.clone()]),
        )];
        let written = settings_xml(&settings, true);
        assert!(written.contains(
            "  <localCorporaSettings>\n    <localCorpusSettings name=\"foo\">\n      \
             <localCorpusPath>bar</localCorpusPath>\n"
        ));
        let read = read_settings(&written).unwrap();
        let Value::LocalCorpora(corpora) = &read[0].1 else {
            panic!("not read back as corpora: {read:?}");
        };
        assert_eq!(corpora[0].name.as_deref(), Some("foo"));
        assert_eq!(corpora[0].paths, ["bar", "baz"]);
        assert_eq!(corpora[0].cache_file_path, corpus.cache_file_path);
    }

    #[test]
    fn special_characters_survive_the_round_trip() {
        let path = "/Applications/Q&A <Beta>/MuseScore 4.app";
        let settings = vec![("musicxmlPath".to_owned(), Value::text(path))];
        let written = settings_xml(&settings, false);
        assert!(written.contains("Q&amp;A &lt;Beta&gt;"));
        assert_eq!(read_settings(&written).unwrap()[0].1, Value::text(path));
    }

    #[test]
    fn a_settings_file_that_is_not_xml_is_an_error() {
        assert!(matches!(
            read_settings("<settings>"),
            Err(Error::Environment(_))
        ));
    }

    #[test]
    fn dot_dot_after_a_missing_directory_is_kept() {
        let cleaned = clean_path("tinyNotation: /../../x.xml");
        assert!(
            cleaned.ends_with("/tinyNotation: /../../x.xml"),
            "{cleaned}"
        );
    }

    #[test]
    fn normalizing_matches_posix_normpath() {
        assert_eq!(normalized("//a/b"), "//a/b");
        assert_eq!(normalized("///a/../b"), "/b");
        assert_eq!(normalized("../a/./b/"), "../a/b");
        assert_eq!(normalized("/.."), "/");
        assert_eq!(normalized("a/.."), ".");
    }

    #[test]
    fn unset_variables_are_left_as_written() {
        assert_eq!(
            expand_variables("/x/$MUSIC21_RS_SURELY_UNSET/${ALSO_UNSET_HERE}"),
            "/x/$MUSIC21_RS_SURELY_UNSET/${ALSO_UNSET_HERE}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_directory_others_can_write_is_passed_over() {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};

        let parent = std::env::temp_dir().join(format!("music21-rs-env-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&parent);
        std::fs::create_dir(&parent).unwrap();
        let shared = parent.join("music21");
        std::fs::create_dir(&shared).unwrap();
        std::fs::set_permissions(&shared, std::fs::Permissions::from_mode(0o777)).unwrap();
        let user = std::fs::metadata(&parent).unwrap().uid();
        let chosen = default_root_temp_dir(&parent, Some(user), is_accessible).unwrap();
        assert_eq!(chosen, parent.join(format!("music21-userid-{user}")));
        let mode = std::fs::metadata(&chosen).unwrap().mode();
        assert_eq!(mode & 0o077, 0);
        std::fs::remove_dir_all(&parent).unwrap();
    }
}
