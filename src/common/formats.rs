//! music21's `common.formats`: the formats music21 reads and writes, by
//! name and by file extension, as its converters register them.

use crate::common::stringtools::is_python_space;

/// The formats `show` takes: music21's `VALID_SHOW_FORMATS`.
pub const VALID_SHOW_FORMATS: [&str; 17] = [
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

/// The formats `write` takes: music21's `VALID_WRITE_FORMATS`, which are
/// the show formats and RomanText.
pub const VALID_WRITE_FORMATS: [&str; 18] = [
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

/// The answers the `autoDownload` setting takes: music21's
/// `VALID_AUTO_DOWNLOAD`.
pub const VALID_AUTO_DOWNLOAD: [&str; 3] = ["ask", "deny", "allow"];

/// What one of music21's converters registers itself for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SubConverter {
    /// The class's name in `music21.converter.subConverters`.
    pub class_name: &'static str,
    /// Its format names, the first being the format's own.
    pub formats: &'static [&'static str],
    /// The extensions it reads, without their dots.
    pub input_extensions: &'static [&'static str],
    /// The extensions it writes, without their dots.
    pub output_extensions: &'static [&'static str],
}

const fn converter(
    class_name: &'static str,
    formats: &'static [&'static str],
    input_extensions: &'static [&'static str],
    output_extensions: &'static [&'static str],
) -> SubConverter {
    SubConverter {
        class_name,
        formats,
        input_extensions,
        output_extensions,
    }
}

/// music21's converters, in the order its `Converter.subConvertersList`
/// gives them where none has been registered: by class name, with the
/// plain `SubConverter` last.
pub const SUB_CONVERTERS: [SubConverter; 20] = [
    converter("ConverterABC", &["abc"], &["abc"], &[]),
    converter("ConverterBraille", &["braille"], &[], &["txt"]),
    converter("ConverterCapella", &["capella"], &["capx"], &[]),
    converter(
        "ConverterClercqTemperley",
        &["cttxt", "har", "clercqTemperley"],
        &["cttxt", "har", "clercqTemperley"],
        &[],
    ),
    converter("ConverterHumdrum", &["humdrum"], &["krn"], &[]),
    converter("ConverterIPython", &["ipython", "jupyter"], &[], &[]),
    converter(
        "ConverterLilypond",
        &["lilypond", "lily"],
        &[],
        &["ly", "png", "pdf", "svg"],
    ),
    converter("ConverterMEI", &["mei"], &["mei"], &[]),
    converter("ConverterMidi", &["midi"], &["mid", "midi"], &["mid"]),
    converter(
        "ConverterMuseData",
        &["musedata"],
        &["md", "musedata", "zip"],
        &[],
    ),
    converter(
        "ConverterMusicXML",
        &["musicxml", "xml"],
        &["xml", "mxl", "musicxml"],
        &["musicxml", "xml", "mxl"],
    ),
    converter("ConverterNoteworthy", &["noteworthytext"], &["nwctxt"], &[]),
    converter("ConverterNoteworthyBinary", &["noteworthy"], &["nwc"], &[]),
    converter(
        "ConverterRomanText",
        &["romantext", "rntext"],
        &["rntxt", "rntext", "romantext", "rtxt"],
        &["rntxt"],
    ),
    converter("ConverterScala", &["scala"], &["scl"], &["scl"]),
    converter("ConverterText", &["text", "txt", "t"], &[], &["txt"]),
    converter("ConverterTextLine", &["textline"], &[], &["txt"]),
    converter(
        "ConverterTinyNotation",
        &["tinynotation"],
        &["tntxt", "tinynotation"],
        &[],
    ),
    converter(
        "ConverterVolpiano",
        &["volpiano"],
        &["volpiano", "vp"],
        &["txt", "vp"],
    ),
    converter("SubConverter", &[], &[], &[]),
];

/// A format name lowered and trimmed, as music21 reads one.
fn cleaned(format: &str) -> String {
    format
        .to_lowercase()
        .trim_matches(is_python_space)
        .to_string()
}

/// The converter registered for a format name, which is not an
/// extension: music21's `findSubConverterForFormat`, by its class.
///
/// ```
/// use music21::common::formats::sub_converter_for_format;
///
/// assert_eq!(sub_converter_for_format("t").unwrap().class_name, "ConverterText");
/// assert!(sub_converter_for_format("mxl").is_none());
/// ```
pub fn sub_converter_for_format(format: &str) -> Option<&'static SubConverter> {
    let format = cleaned(format);
    SUB_CONVERTERS
        .iter()
        .find(|converter| converter.formats.contains(&format.as_str()))
}

/// The format a name, an abbreviation or an extension stands for, with or
/// without its dot and with anything after a further dot ignored: music21's
/// `Converter.regularizeFormat`.
///
/// ```
/// use music21::common::formats::regularize_format;
///
/// assert_eq!(regularize_format("mxl"), Some("musicxml"));
/// assert_eq!(regularize_format("lily.png"), Some("lilypond"));
/// assert_eq!(regularize_format("blah"), None);
/// ```
pub fn regularize_format(format: &str) -> Option<&'static str> {
    let format = cleaned(format);
    let format = format.strip_prefix('.').unwrap_or(&format);
    let format = format.split('.').next().unwrap_or_default();
    let found = SUB_CONVERTERS
        .iter()
        .find(|converter| converter.formats.contains(&format))
        .or_else(|| {
            SUB_CONVERTERS
                .iter()
                .find(|converter| converter.input_extensions.contains(&format))
        })?;
    found.formats.first().copied()
}

/// The format a name or an extension stands for and the extension it is
/// written with, dot and all: music21's `findFormat`.
///
/// ```
/// use music21::common::formats::find_format;
///
/// assert_eq!(find_format(".mxl"), (Some("musicxml"), Some(".musicxml".to_string())));
/// assert_eq!(find_format("abc"), (Some("abc"), Some(".abc".to_string())));
/// assert_eq!(find_format("wpd"), (None, None));
/// ```
pub fn find_format(format: &str) -> (Option<&'static str>, Option<String>) {
    let Some(name) = regularize_format(format) else {
        return (None, None);
    };
    let first_lowered = |converter: &&SubConverter| {
        converter
            .formats
            .iter()
            .any(|registered| registered.to_lowercase() == name)
    };
    let extension = SUB_CONVERTERS
        .iter()
        .find(first_lowered)
        .and_then(|converter| {
            converter
                .output_extensions
                .first()
                .or(converter.input_extensions.first())
                .map(|extension| format!(".{extension}"))
        });
    (Some(name), extension)
}

/// Each extension, dotted, that a format reads, the format given by name
/// or by one of its extensions: music21's `findInputExtension`.
///
/// ```
/// use music21::common::formats::find_input_extension;
///
/// assert_eq!(find_input_extension("musicxml"), [".xml", ".mxl", ".musicxml"]);
/// assert_eq!(find_input_extension(".mxl"), [".xml", ".mxl", ".musicxml"]);
/// assert!(find_input_extension("blah").is_empty());
/// ```
pub fn find_input_extension(format: &str) -> Vec<String> {
    let format = cleaned(format);
    let format = format.strip_prefix('.').unwrap_or(&format);
    let dotted = |converter: &SubConverter| -> Vec<String> {
        converter
            .input_extensions
            .iter()
            .map(|extension| {
                if extension.starts_with('.') {
                    (*extension).to_string()
                } else {
                    format!(".{extension}")
                }
            })
            .collect()
    };
    if let Some(converter) = sub_converter_for_format(format) {
        return dotted(converter);
    }
    SUB_CONVERTERS
        .iter()
        .find(|converter| converter.input_extensions.contains(&format))
        .map(dotted)
        .unwrap_or_default()
}

/// The format and extension a URL names, read from its query or from how
/// it ends: music21's `findFormatExtURL`.
///
/// ```
/// use music21::common::formats::find_format_ext_url;
///
/// let url = "http://somesite.com/cgi-bin/ksdata?l=cc/schubert/d0576&file=d0576-06.krn&f=kern";
/// assert_eq!(find_format_ext_url(url), (Some("humdrum"), Some(".krn".to_string())));
/// assert_eq!(find_format_ext_url("https://junk"), (None, None));
/// ```
pub fn find_format_ext_url(url: &str) -> (Option<&'static str>, Option<String>) {
    let extension = if url.contains("=xml") {
        Some(".xml".to_string())
    } else if url.contains("=kern") {
        Some(".krn".to_string())
    } else if url.contains("format=stage2") || url.contains("format=stage1") {
        Some(".md".to_string())
    } else {
        // Each converter's first extension the URL ends with, the last
        // converter that has one winning, as music21's loop leaves it.
        SUB_CONVERTERS
            .iter()
            .filter_map(|converter| {
                converter
                    .input_extensions
                    .iter()
                    .find(|extension| url.ends_with(&format!(".{extension}")))
            })
            .next_back()
            .map(|extension| format!(".{extension}"))
    };
    match extension {
        Some(extension) => (find_format(&extension).0, Some(extension)),
        None => (None, None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// music21's own examples for `findFormat`.
    #[test]
    fn formats_are_found_as_music21_finds_them() {
        let cases = [
            ("musicxml", "musicxml", ".musicxml"),
            ("lily", "lilypond", ".ly"),
            ("lily.png", "lilypond", ".ly"),
            ("humdrum", "humdrum", ".krn"),
            ("txt", "text", ".txt"),
            ("textline", "textline", ".txt"),
            ("midi", "midi", ".mid"),
            ("scl", "scala", ".scl"),
            ("braille", "braille", ".txt"),
            ("capx", "capella", ".capx"),
            ("md", "musedata", ".md"),
            (".md", "musedata", ".md"),
        ];
        for (given, format, extension) in cases {
            assert_eq!(
                find_format(given),
                (Some(format), Some(extension.to_string())),
                "{given}"
            );
        }
    }

    #[test]
    fn a_format_registered_with_a_capital_is_not_found_lowered() {
        assert_eq!(regularize_format("clercqTemperley"), None);
        assert_eq!(regularize_format("cttxt"), Some("cttxt"));
    }

    #[test]
    fn urls_are_read_by_query_then_ending() {
        let url = "http://somesite.com/?l=cc/schubert/piano/d0576&file=d0576-06.krn&f=xml";
        assert_eq!(
            find_format_ext_url(url),
            (Some("musicxml"), Some(".xml".to_string()))
        );
        assert_eq!(
            find_format_ext_url("http://somesite.com/files/mid001.mid"),
            (Some("midi"), Some(".mid".to_string()))
        );
    }
}
