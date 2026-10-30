// MuseScore run as a converter. A score is handed across as MusicXML: to
// read a file MuseScore writes it out as MusicXML and `from_musicxml` reads
// that; to write one, `to_musicxml` writes the score and MuseScore converts
// it. This is the one part of the crate that runs a program and touches
// files, which is why it sits behind the `musescore` feature and is left out
// of a wasm build.

use crate::error::{Error, Result};
use crate::musicxml::{ExportOptions, from_musicxml, to_musicxml};
use crate::stream::Stream;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

/// The environment variable naming the MuseScore executable, where it is
/// not where [`MuseScore::find`] looks.
pub const MUSESCORE_PATH: &str = "MUSESCORE_PATH";

/// The names MuseScore's executable goes by on a search path.
const NAMES: [&str; 7] = [
    "MuseScore4",
    "mscore4portable",
    "musescore4",
    "mscore",
    "musescore",
    "MuseScore3",
    "mscore3",
];

/// A MuseScore installation.
///
/// ```no_run
/// use music21_rs::musescore::MuseScore;
/// use std::path::Path;
///
/// let musescore = MuseScore::find().expect("MuseScore is installed");
/// let score = musescore.read(Path::new("quartet.mscz"))?;
/// musescore.write(&score, Path::new("quartet.pdf"), &Default::default())?;
/// # Ok::<(), music21_rs::Error>(())
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MuseScore {
    executable: PathBuf,
}

fn failed(message: impl Into<String>) -> Error {
    Error::MuseScore(message.into())
}

/// A directory in the system's temporary directory that this call made and
/// only this user can open, removed with what is in it when the call is
/// over. The temporary directory may be shared with other users, so the
/// files a call writes and reads back go in here, where no one else can put
/// a file or a link in their place.
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Result<Self> {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        // Elsewhere the temporary directory is the user's own.
        #[cfg(not(unix))]
        let builder = std::fs::DirBuilder::new();
        #[cfg(unix)]
        let builder = {
            use std::os::unix::fs::DirBuilderExt;
            let mut builder = std::fs::DirBuilder::new();
            builder.mode(0o700);
            builder
        };
        loop {
            let serial = COUNTER.fetch_add(1, Ordering::Relaxed);
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |elapsed| elapsed.subsec_nanos());
            let path = std::env::temp_dir().join(format!(
                "music21-rs-{}-{serial}-{nanos}",
                std::process::id()
            ));
            // Creating the directory fails where anything, a link included,
            // is already there, so the one made is new.
            match builder.create(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => {
                    return Err(failed(format!(
                        "no scratch directory could be made in {}: {error}",
                        path.display()
                    )));
                }
            }
        }
    }

    /// A file of this name in the directory.
    fn file(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

impl MuseScore {
    /// The installation whose executable is at this path.
    pub fn at(executable: impl Into<PathBuf>) -> Self {
        Self {
            executable: executable.into(),
        }
    }

    /// Looks for MuseScore: at the path [`MUSESCORE_PATH`] names, then on
    /// the search path, then where its installers put it.
    pub fn find() -> Option<Self> {
        if let Some(named) = std::env::var_os(MUSESCORE_PATH) {
            let path = PathBuf::from(named);
            if path.is_file() {
                return Some(Self::at(path));
            }
        }
        let suffix = std::env::consts::EXE_SUFFIX;
        if let Some(paths) = std::env::var_os("PATH") {
            for directory in std::env::split_paths(&paths) {
                for name in NAMES {
                    let candidate = directory.join(format!("{name}{suffix}"));
                    if candidate.is_file() {
                        return Some(Self::at(candidate));
                    }
                }
            }
        }
        let installed: [&str; 6] = [
            r"C:\Program Files\MuseScore 4\bin\MuseScore4.exe",
            r"C:\Program Files\MuseScore 3\bin\MuseScore3.exe",
            "/Applications/MuseScore 4.app/Contents/MacOS/mscore",
            "/Applications/MuseScore 3.app/Contents/MacOS/mscore",
            "/usr/bin/mscore",
            "/usr/local/bin/mscore",
        ];
        installed
            .iter()
            .map(Path::new)
            .find(|path| path.is_file())
            .map(Self::at)
    }

    /// Where the executable is.
    pub fn executable(&self) -> &Path {
        &self.executable
    }

    /// Converts one file into another, each in the format its extension
    /// names: `mscore -o output input`.
    pub fn convert(&self, input: &Path, output: &Path) -> Result<()> {
        if !input.is_file() {
            return Err(failed(format!("{} is not a file", input.display())));
        }
        let ran = Command::new(&self.executable)
            .arg("-o")
            .arg(output)
            .arg(input)
            .output()
            .map_err(|error| {
                failed(format!(
                    "{} could not be run: {error}",
                    self.executable.display()
                ))
            })?;
        if !ran.status.success() || !output.is_file() {
            let said = String::from_utf8_lossy(&ran.stderr);
            return Err(failed(format!(
                "MuseScore could not convert {} to {}{}{}",
                input.display(),
                output.display(),
                if said.trim().is_empty() { "" } else { ": " },
                said.trim()
            )));
        }
        Ok(())
    }

    /// Reads a file of any format MuseScore opens -- `.mscz`, `.mscx`,
    /// `.gp`, `.cap`, `.mei`, `.mid`, `.mxl` -- as a score.
    pub fn read(&self, path: &Path) -> Result<Stream> {
        let scratch = Scratch::new()?;
        let converted = scratch.file("score.musicxml");
        self.convert(path, &converted)?;
        let text = std::fs::read_to_string(&converted)
            .map_err(|error| failed(format!("what MuseScore wrote could not be read: {error}")))?;
        from_musicxml(&text)
    }

    /// Writes a score as a file of any format MuseScore saves, named by the
    /// path's extension: `.mscz`, `.mscx`, `.pdf`, `.png`, `.svg`, `.mid`,
    /// `.mp3`, `.mei`.
    pub fn write(&self, score: &Stream, path: &Path, options: &ExportOptions) -> Result<()> {
        let scratch = Scratch::new()?;
        let written = scratch.file("score.musicxml");
        std::fs::write(&written, to_musicxml(score, options)?)
            .map_err(|error| failed(format!("the score could not be written out: {error}")))?;
        self.convert(&written, path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Clef, Note, StreamKind, TimeSignature};

    #[test]
    fn a_missing_program_and_a_missing_file_are_errors() {
        let nowhere = MuseScore::at("no-such-directory/mscore");
        let missing = Path::new("no-such-file.mscz");
        assert!(matches!(
            nowhere.convert(missing, Path::new("out.musicxml")),
            Err(Error::MuseScore(_))
        ));
        let scratch = Scratch::new().unwrap();
        let here = scratch.file("score.musicxml");
        std::fs::write(&here, "<score-partwise/>").unwrap();
        let ran = nowhere.convert(&here, &scratch.file("score.mscz"));
        assert!(
            matches!(ran, Err(Error::MuseScore(message)) if message.contains("could not be run"))
        );
    }

    #[test]
    fn scratch_directories_are_new_private_and_removed() {
        let first = Scratch::new().unwrap();
        let second = Scratch::new().unwrap();
        assert_ne!(first.0, second.0);
        for scratch in [&first, &second] {
            let found = std::fs::symlink_metadata(&scratch.0).unwrap();
            assert!(found.is_dir());
            assert_eq!(std::fs::read_dir(&scratch.0).unwrap().count(), 0);
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                assert_eq!(found.permissions().mode() & 0o077, 0);
            }
        }
        let gone = first.0.clone();
        std::fs::write(first.file("score.musicxml"), "<score-partwise/>").unwrap();
        drop(first);
        assert!(!gone.exists());
    }

    /// Round the houses through a real MuseScore, where there is one.
    #[test]
    fn a_score_survives_a_trip_through_musescore() {
        let Some(musescore) = MuseScore::find() else {
            // Nothing to test against on a machine without MuseScore.
            return;
        };
        let mut measure = Stream::with_kind(StreamKind::Measure);
        measure.set_number(1);
        measure.insert(0.0, Clef::treble());
        measure.insert(0.0, TimeSignature::new(3, 4).unwrap());
        for name in ["C4", "E4", "G4"] {
            measure.push(Note::from_name(name).unwrap());
        }
        let mut part = Stream::with_kind(StreamKind::Part);
        part.push(measure);
        let mut score = Stream::with_kind(StreamKind::Score);
        score.push(part);

        let scratch = Scratch::new().unwrap();
        let file = scratch.file("score.mscz");
        musescore
            .write(&score, &file, &ExportOptions::default())
            .unwrap();
        let read = musescore.read(&file).unwrap();
        let names: Vec<String> = read
            .pitches()
            .iter()
            .map(|pitch| pitch.name_with_octave())
            .collect();
        assert_eq!(names, ["C4", "E4", "G4"]);
    }
}
