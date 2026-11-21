//! The core corpus's directories and the works in each: music21's
//! `corpus.work` and `Corpus.getComposer`.

use std::path::{Path, PathBuf};

use crate::common::formats::find_format;
use crate::common::stringtools::{space_camel_case, title_case};

/// What music21 keeps together when it spaces a camel-cased name.
const KEPT_TOGETHER: [&str; 1] = ["PMFC"];

/// One directory of the corpus: its name on disk, the composer or
/// collection it holds, and whether that is a composer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Directory {
    /// The directory's name.
    pub name: &'static str,
    /// The composer's or collection's name.
    pub title: &'static str,
    /// Whether the directory holds one composer's works.
    pub holds: Holder,
}

/// Whose works a directory holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Holder {
    /// One composer's.
    Composer,
    /// A collection's, of several or unknown composers.
    Collection,
}

const fn composer(name: &'static str, title: &'static str) -> Directory {
    Directory {
        name,
        title,
        holds: Holder::Composer,
    }
}

const fn collection(name: &'static str, title: &'static str) -> Directory {
    Directory {
        name,
        title,
        holds: Holder::Collection,
    }
}

/// The core corpus's directories, in music21's order: its
/// `CoreCorpus._directoryInformation`.
pub const CORE_DIRECTORIES: [Directory; 35] = [
    collection("airdsAirs", "Aird's Airs"),
    composer("bach", "Johann Sebastian Bach"),
    composer("beach", "Amy Beach"),
    composer("beethoven", "Ludwig van Beethoven"),
    composer("chopin", "Frederic Chopin"),
    composer("ciconia", "Johannes Ciconia"),
    composer("corelli", "Arcangelo Corelli"),
    composer("cpebach", "C.P.E. Bach"),
    collection("demos", "Demonstration Files"),
    collection("essenFolksong", "Essen Folksong Collection"),
    composer("handel", "George Frideric Handel"),
    composer("haydn", "Joseph Haydn"),
    composer("joplin", "Scott Joplin"),
    composer("johnson_j_r", "J. Rosamund Johnson"),
    composer("josquin", "Josquin des Prez"),
    collection("leadSheet", "Leadsheet demos"),
    composer("liliuokalani", "Queen Liliʻuokalani"),
    composer("luca", "D. Luca"),
    composer("lusitano", "Vicente Lusitano"),
    collection("miscFolk", "Miscellaneous Folk"),
    composer("monteverdi", "Claudio Monteverdi"),
    composer("mozart", "Wolfgang Amadeus Mozart"),
    collection("nottingham-dataset", "Nottingham Music Database (partial)"),
    collection("oneills1850", "Oneill's 1850 Collection"),
    composer("palestrina", "Giovanni Palestrina"),
    collection("ryansMammoth", "Ryan's Mammoth Collection"),
    composer("schoenberg", "Arnold Schoenberg"),
    composer("schubert", "Franz Schubert"),
    composer("schumann_robert", "Robert Schumann"),
    composer("schumann_clara", "Clara Schumann"),
    collection("theoryExercises", "Theory Exercises"),
    collection("trecento", "Fourteenth-Century Italian Music"),
    composer("verdi", "Giuseppe Verdi"),
    composer("weber", "Carl Maria von Weber"),
    composer("webern", "Anton Webern"),
];

/// One file of a work: music21's `CorpusFile`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CorpusFile {
    /// The path under the corpus, from the directory on.
    pub path: String,
    /// A title made from the file's name.
    pub title: String,
    /// The file's name.
    pub filename: String,
    /// The file's format.
    pub format: &'static str,
    /// The file's extension, with its dot.
    pub ext: String,
}

/// A work of a directory and its files: music21's `CorpusWork`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CorpusWork {
    /// The work's name on disk: the subdirectory, or the file without its
    /// extension.
    pub stub: String,
    /// A title made from the stub.
    pub title: String,
    /// The work's files, in the order given.
    pub files: Vec<CorpusFile>,
}

/// The paths that belong to a composer or collection: those with a part,
/// or a part's name before its last dot, of that name, ignoring case.
/// Sorted, as music21's `getComposer` answers them.
///
/// ```
/// use std::path::PathBuf;
/// use music21::corpus::work::composer_paths;
///
/// let paths = [PathBuf::from("/c/bach/bwv66.6.mxl"), PathBuf::from("/c/beach/x.mxl")];
/// assert_eq!(composer_paths("Bach", &paths), [PathBuf::from("/c/bach/bwv66.6.mxl")]);
/// ```
pub fn composer_paths(name: &str, paths: &[PathBuf]) -> Vec<PathBuf> {
    let name = name.to_lowercase();
    let mut found: Vec<PathBuf> = paths
        .iter()
        .filter(|path| names_composer(path, &name))
        .cloned()
        .collect();
    found.sort();
    found
}

fn names_composer(path: &Path, name: &str) -> bool {
    path.components().any(|part| {
        let part = part.as_os_str().to_string_lossy().to_lowercase();
        if part == name {
            return true;
        }
        match part.rsplit_once('.') {
            Some((stem, _)) => stem == name,
            None => false,
        }
    })
}

/// The works a directory's files make, in the order their files first come:
/// music21's `DirectoryInformation.findWorks`. A file is placed after the
/// first place the directory's name appears in its path; a file whose
/// format is not known is left out.
///
/// ```
/// use std::path::PathBuf;
/// use music21::corpus::work::find_works;
///
/// let paths = [
///     PathBuf::from("/c/schoenberg/opus19/movement2.mxl"),
///     PathBuf::from("/c/schoenberg/opus19/movement6.mxl"),
/// ];
/// let works = find_works("schoenberg", &paths);
/// assert_eq!(works[0].title, "Opus 19");
/// assert_eq!(works[0].files[1].path, "schoenberg/opus19/movement6.mxl");
/// assert_eq!(works[0].files[1].title, "Movement 6");
/// ```
pub fn find_works(directory: &str, paths: &[PathBuf]) -> Vec<CorpusWork> {
    let mut works: Vec<CorpusWork> = Vec::new();
    for path in paths {
        let text = path.to_string_lossy();
        let Some((_, stub)) = text.split_once(directory) else {
            continue;
        };
        // Windows takes either separator, and a path can mix them.
        let separators = ['/', std::path::MAIN_SEPARATOR];
        let stub = stub.strip_prefix(separators).unwrap_or(stub);
        let components: Vec<&str> = stub.split(separators).collect();
        let filename = components.last().copied().unwrap_or_default();
        let ext = suffix(filename);
        let Some(format) = find_format(ext).0 else {
            continue;
        };

        // The work is the first component, a file's extension removed.
        let work_stub = components[0].replace(ext, "");
        let at = match works.iter().position(|work| work.stub == work_stub) {
            Some(at) => at,
            None => {
                works.push(CorpusWork {
                    title: titled(&work_stub),
                    stub: work_stub,
                    files: Vec::new(),
                });
                works.len() - 1
            }
        };
        works[at].files.push(CorpusFile {
            path: Path::new(directory)
                .join(stub)
                .to_string_lossy()
                .into_owned(),
            title: titled(&filename.replace(ext, "")),
            filename: filename.to_string(),
            format,
            ext: ext.to_string(),
        });
    }
    works
}

/// A name spaced at its case changes and numbers, in title case.
fn titled(name: &str) -> String {
    title_case(&space_camel_case(name, true, &KEPT_TOGETHER))
}

/// A file name's extension with its dot, as `pathlib`'s `suffix` finds it.
fn suffix(name: &str) -> &str {
    match name.rfind('.') {
        Some(at) if at > 0 && at + 1 < name.len() => &name[at..],
        _ => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_named_like_the_composer_is_found() {
        let paths = [
            PathBuf::from("/c/demos/bach.krn"),
            PathBuf::from("/c/demos/x.krn"),
        ];
        assert_eq!(
            composer_paths("bach", &paths),
            [PathBuf::from("/c/demos/bach.krn")]
        );
    }

    #[test]
    fn a_file_of_no_known_format_makes_no_work() {
        let paths = [
            PathBuf::from("/c/verdi/score.pdf"),
            PathBuf::from("/c/verdi/x"),
        ];
        assert!(find_works("verdi", &paths).is_empty());
    }

    #[test]
    fn a_file_at_the_top_is_a_work_of_its_own() {
        let works = find_works("beach", &[PathBuf::from("/c/beach/prayer.mxl")]);
        assert_eq!(works[0].stub, "prayer");
        assert_eq!(works[0].files[0].ext, ".mxl");
    }
}
