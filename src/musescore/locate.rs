//! Where MuseScore is installed, and the numbered pages it writes when it
//! turns a score into PNG images: music21's `converter.museScore`, less the
//! running of the program. It reads directories and nothing else.

use std::path::{Path, PathBuf};

use crate::environment::Platform;

/// What a PNG file's name ends with.
const PNG: &str = ".png";

/// The page numbers MuseScore puts after a `-` on the first page, by how
/// many pages it wrote: `-1`, `-01` and so on.
const FIRST_PAGES: [&str; 5] = ["1", "01", "001", "0001", "00001"];

/// The executable inside each MuseScore installation, as a pattern under
/// a directory it is installed in, newest naming first where a name does
/// not say which version it is.
pub fn patterns(platform: Platform) -> &'static [&'static str] {
    match platform {
        Platform::Darwin => &["MuseScore*.app/Contents/MacOS/mscore"],
        Platform::Windows => &["MuseScore*/bin/MuseScore*.exe"],
        // A bare `mscore` is MuseScore 2 on Debian but MuseScore 4 on Arch.
        Platform::Nix => &[
            "mscore4portable",
            "mscore3",
            "musescore3",
            "mscore",
            "musescore",
        ],
    }
}

/// The directories MuseScore is looked for in when none are given: the
/// applications folders on a Mac, the program folders on Windows and the
/// search path elsewhere.
pub fn default_directories(platform: Platform, home: &Path) -> Vec<PathBuf> {
    match platform {
        Platform::Darwin => vec![PathBuf::from("/Applications"), home.join("Applications")],
        Platform::Windows => ["ProgramFiles", "ProgramFiles(x86)", "ProgramW6432"]
            .iter()
            .filter_map(std::env::var_os)
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .collect(),
        Platform::Nix => std::env::var_os("PATH")
            .map(|paths| {
                std::env::split_paths(&paths)
                    .filter(|path| !path.as_os_str().is_empty())
                    .collect()
            })
            .unwrap_or_default(),
    }
}

/// Whether a name is of MuseScore 1 or 2, which cannot make the images:
/// music21's `MuseScore ?[12]\b`, any case.
pub fn is_before_three(name: &str) -> bool {
    let lower = name.to_lowercase();
    let mut rest = lower.as_str();
    while let Some(at) = rest.find("musescore") {
        let after = &rest[at + "musescore".len()..];
        let digits = after.strip_prefix(' ').unwrap_or(after);
        let mut chars = digits.chars();
        if let Some('1' | '2') = chars.next() {
            let bounded = chars
                .next()
                .is_none_or(|next| !(next.is_alphanumeric() || next == '_'));
            if bounded {
                return true;
            }
        }
        rest = after;
    }
    false
}

/// MuseScore 3 or newer, found in the directories given, or nowhere: the
/// newest where several are, by the name of the path under its directory.
/// music21's `findMuseScore`.
pub fn find(platform: Platform, directories: &[PathBuf]) -> Option<PathBuf> {
    for pattern in patterns(platform) {
        // Keyed by the path under its directory; a later directory's path
        // of the same name takes its place, as music21's dictionary does.
        let mut found: Vec<(String, PathBuf)> = Vec::new();
        for directory in directories {
            for path in glob(directory, pattern) {
                let name = relative_name(directory, &path);
                found.retain(|(held, _)| held != &name);
                found.push((name, path));
            }
        }
        let newest = found
            .into_iter()
            .filter(|(name, _)| !is_before_three(name))
            .max_by(|left, right| left.0.cmp(&right.0));
        if let Some((_, path)) = newest {
            return Some(path);
        }
    }
    None
}

/// A path's components under a directory, joined by the platform's
/// separator, as `str(path.relative_to(directory))` writes them.
fn relative_name(directory: &Path, path: &Path) -> String {
    path.strip_prefix(directory)
        .unwrap_or(path)
        .to_string_lossy()
        .into_owned()
}

/// Every path under a directory a pattern matches, a component at a time,
/// where `*` in a component stands for any run of characters.
fn glob(directory: &Path, pattern: &str) -> Vec<PathBuf> {
    let mut reached = vec![directory.to_path_buf()];
    for component in pattern.split('/') {
        let mut next = Vec::new();
        for path in &reached {
            if !component.contains('*') {
                let joined = path.join(component);
                if joined.exists() {
                    next.push(joined);
                }
                continue;
            }
            let Ok(entries) = std::fs::read_dir(path) else {
                continue;
            };
            let mut matched: Vec<PathBuf> = entries
                .flatten()
                .filter(|entry| wildcard(component, &entry.file_name().to_string_lossy()))
                .map(|entry| entry.path())
                .collect();
            matched.sort();
            next.extend(matched);
        }
        reached = next;
    }
    reached
}

/// Whether a name matches a pattern whose `*`s stand for any run of
/// characters.
fn wildcard(pattern: &str, name: &str) -> bool {
    let mut parts = pattern.split('*');
    let first = parts.next().unwrap_or_default();
    let Some(mut rest) = name.strip_prefix(first) else {
        return false;
    };
    let parts: Vec<&str> = parts.collect();
    let Some((last, middle)) = parts.split_last() else {
        return rest.is_empty();
    };
    for part in middle {
        let Some(at) = rest.find(part) else {
            return false;
        };
        rest = &rest[at + part.len()..];
    }
    rest.len() >= last.len() && rest.ends_with(last)
}

/// A path named in a refusal as music21 names it.
fn shown(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// The first page MuseScore wrote for an image asked for at a path ending
/// in `.png`: `score-1.png`, `score-01.png` and so on, whichever is there.
/// music21's `findNumberedPNGPath`.
pub fn first_page(path: &Path) -> Result<PathBuf, PageError> {
    let written = shown(path);
    let Some(stem) = written.strip_suffix(PNG) else {
        return Err(PageError::NotPng(format!(
            "inputFp must end with \".png\"; got {written}"
        )));
    };
    for number in FIRST_PAGES {
        let page = PathBuf::from(format!("{stem}-{number}{PNG}"));
        if page.exists() {
            return Ok(page);
        }
    }
    Err(PageError::Missing(format!(
        "No png file for {written} (such as {stem}-1.png) was found.  The conversion to png failed"
    )))
}

/// Why a page could not be found or read, each with music21's message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PageError {
    /// The name does not end in `.png`.
    NotPng(String),
    /// The name has no `-` before its number.
    Unnumbered(String),
    /// What stands where the last page's number should is no number.
    NoNumber(String),
    /// No page was written.
    Missing(String),
}

/// The last page MuseScore wrote beside a numbered one: for `abc-01.png`,
/// the last `abc-*.png` by name. music21's `findLastPNGPath`.
pub fn last_page(page: &Path) -> Result<PathBuf, PageError> {
    let name = file_name(page);
    if !name.ends_with(PNG) {
        return Err(PageError::NotPng(format!(
            "inputFp must end with \".png\"; got {}",
            shown(page)
        )));
    }
    let Some(dash) = name.rfind('-') else {
        return Err(PageError::Unnumbered(format!(
            "inputFp must end with \"-0001.png\" or similar; got {}",
            shown(page)
        )));
    };
    let prefix = format!("{}-", &name[..dash]);
    let directory = page.parent().unwrap_or(Path::new("."));
    let mut pages: Vec<PathBuf> = std::fs::read_dir(directory)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            name.starts_with(&prefix) && name.ends_with(PNG) && name.len() >= prefix.len() + 4
        })
        .map(|entry| entry.path())
        .collect();
    pages.sort_by_key(|path| file_name(path));
    // The page asked about is among them, so there is a last.
    Ok(pages.pop().unwrap_or_else(|| page.to_path_buf()))
}

/// How many pages MuseScore wrote and how many digits number them, from
/// the first page and the last: music21's `findPNGRange`.
pub fn page_range(first: &Path, last: &Path) -> Result<PageRange, PageError> {
    let name = file_name(first);
    if !name.ends_with(PNG) {
        return Err(PageError::NotPng(format!(
            "firstFp must end with \".png\"; got {}",
            shown(first)
        )));
    }
    let Some(dash) = name.rfind('-') else {
        return Err(PageError::Unnumbered(format!(
            "firstFp must end with \"-0001.png\" or similar; got {}",
            shown(first)
        )));
    };
    let digits = name.len() - dash - PNG.len() - 1;
    let last_name = file_name(last);
    let number = last_name
        .get(dash + 1..dash + 1 + digits)
        .unwrap_or_default();
    let last = number.parse().map_err(|_| {
        PageError::NoNumber(format!(
            "invalid literal for int() with base 10: '{number}'"
        ))
    })?;
    Ok(PageRange { last, digits })
}

/// The number of the last page and how many digits each page's number has.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PageRange {
    /// The number of the last page.
    pub last: u64,
    /// How many digits each number is written with.
    pub digits: usize,
}

/// A path's file name as text.
fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// How MuseScore is asked to write a score as an image.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImageRequest {
    /// The image's resolution, MuseScore's own where none.
    pub dpi: Option<u32>,
    /// The pixels left around the music where the image is cut to it, or
    /// none to leave the page whole.
    pub trim: Option<u32>,
}

/// The command line that has MuseScore write `input` as `output`: music21's
/// `runThroughMuseScore`, `-T` for trimming and `-r` for resolution.
pub fn image_command(
    executable: &Path,
    input: &Path,
    output: &Path,
    request: &ImageRequest,
) -> Vec<String> {
    let mut command = vec![shown(executable), shown(input), "-o".into(), shown(output)];
    if let Some(margin) = request.trim {
        command.extend(["-T".into(), margin.to_string()]);
    }
    if let Some(dpi) = request.dpi {
        command.extend(["-r".into(), dpi.to_string()]);
    }
    command
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(root: &Path, relative: &str) {
        let path = root.join(relative);
        std::fs::create_dir_all(path.parent().expect("a parent")).expect("a directory");
        std::fs::write(path, b"").expect("a file");
    }

    fn scratch(name: &str) -> PathBuf {
        let directory =
            std::env::temp_dir().join(format!("music21-rs-locate-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).expect("a directory");
        directory
    }

    #[test]
    fn names_of_musescore_two_are_too_old() {
        assert!(is_before_three("MuseScore 2.app/Contents/MacOS/mscore"));
        assert!(is_before_three("musescore1"));
        assert!(!is_before_three("MuseScore 4.app/Contents/MacOS/mscore"));
        assert!(!is_before_three("MuseScore 20"));
        assert!(!is_before_three("mscore3"));
    }

    #[test]
    fn the_newest_mac_installation_is_found() {
        let apps = scratch("mac");
        touch(&apps, "MuseScore 3.app/Contents/MacOS/mscore");
        touch(&apps, "MuseScore 4.app/Contents/MacOS/mscore");
        touch(&apps, "old/MuseScore 2.app/Contents/MacOS/mscore");
        assert_eq!(
            find(Platform::Darwin, std::slice::from_ref(&apps)),
            Some(apps.join("MuseScore 4.app/Contents/MacOS/mscore"))
        );
        assert_eq!(find(Platform::Darwin, &[apps.join("old")]), None);
        let _ = std::fs::remove_dir_all(apps);
    }

    #[test]
    fn a_name_saying_its_version_comes_before_a_bare_one() {
        let bin = scratch("nix");
        touch(&bin, "mscore");
        touch(&bin, "mscore3");
        assert_eq!(
            find(Platform::Nix, std::slice::from_ref(&bin)),
            Some(bin.join("mscore3"))
        );
        let _ = std::fs::remove_dir_all(bin);
    }

    #[test]
    fn pages_are_found_by_their_numbers() {
        let directory = scratch("pages");
        touch(&directory, "score-01.png");
        touch(&directory, "score-02.png");
        touch(&directory, "score-12.png");
        let first = first_page(&directory.join("score.png")).expect("a first page");
        assert_eq!(first, directory.join("score-01.png"));
        let last = last_page(&first).expect("a last page");
        assert_eq!(last, directory.join("score-12.png"));
        assert_eq!(
            page_range(&first, &last),
            Ok(PageRange {
                last: 12,
                digits: 2
            })
        );
        assert!(first_page(&directory.join("other.png")).is_err());
        assert!(first_page(&directory.join("score.pdf")).is_err());
        let _ = std::fs::remove_dir_all(directory);
    }

    #[test]
    fn an_image_command_trims_and_sets_a_resolution() {
        let request = ImageRequest {
            dpi: Some(72),
            trim: Some(0),
        };
        let command = image_command(
            Path::new("mscore"),
            Path::new("a.xml"),
            Path::new("a.png"),
            &request,
        );
        assert_eq!(
            command,
            ["mscore", "a.xml", "-o", "a.png", "-T", "0", "-r", "72"]
        );
    }
}
