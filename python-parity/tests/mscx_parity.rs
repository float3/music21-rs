//! The crate's reader of MuseScore's own files against MuseScore itself.
//!
//! music21 does not read a `.mscx`, so there is no reader of its to hold
//! this one to. MuseScore is the reference instead: each score under
//! `tests/mscx/` is there twice, as the `.mscx` MuseScore saved and as the
//! MusicXML MuseScore exported from that very file. The first is read by
//! `musescore::from_mscx` and the second by `musicxml::from_musicxml`, both
//! are written out by `to_musicxml`, and the two texts must be the same. The
//! MusicXML reader and writer are held to music21 by their own tests, so the
//! texts agree only where the native reader made of the file what MuseScore
//! itself says is in it.
//!
//! The scores are compared as an outline first -- every part, measure and
//! voice, and in each the elements in order -- because an outline says
//! plainly what differs, and then as MusicXML.
//!
//! No MuseScore is needed to run this, and no Python. The files were made
//! with MuseScore Studio 4.7.5:
//!
//! ```text
//! MuseScore4 -o score.mscx score.mxl        # a music21 corpus score
//! MuseScore4 -o score.musicxml score.mscx   # its own export of that file
//! ```
//!
//! `features`, `pickup` and `arpeggios` were written for this test rather
//! than taken from the corpus, to reach what the corpus scores here do not.
//!
//! # What is not compared
//!
//! MuseScore's export says some things its file does not, and they are
//! taken out of both sides rather than guessed at:
//!
//! - **Stems.** The export gives every note the stem direction MuseScore's
//!   layout chose. The file gives one only where somebody fixed it, on the
//!   note or on its beam. The stems the file fixes are compared; the others
//!   are taken off the export's side.
//! - **The slash of a grace note.** The export says `slash="yes"` on an
//!   acciaccatura and nothing on any other grace note, and a MusicXML
//!   reader takes a grace note that says nothing as slashed. The file says
//!   which kind each is, and the reader keeps that.
//! - **A single part's hidden name.** Whether a score of one part prints
//!   the part's name is its style's to say, and MuseScore 4 keeps the style
//!   in a file of its own beside the `.mscx`. The reader is not given it
//!   and takes MuseScore's default, which hides the name; a score here
//!   whose style sheet shows it says so in the list below.
//! - **A flat or sharp sign in a part's name.** The file names a part
//!   `Clarinet in B♭`; the export writes the sign as a letter in the name
//!   and keeps the sign for a second, display name the MusicXML reader does
//!   not read.
//!
//! # What differs
//!
//! Where MuseScore's export and its own file say different things, the
//! reader follows the file. Such a score is listed as differing, with the
//! reason, and the test fails if it stops differing -- so the list cannot
//! go stale -- as well as if a score listed as agreeing differs.
//!
//! `MSCX_PARITY_DIR` names another directory to compare instead: every
//! `name.mscx` in it that has a `name.musicxml` beside it, each expected to
//! agree. That is how a score is tried before it is added here.

use music21_rs::musescore::from_mscx;
use music21_rs::musicxml::{ExportOptions, from_musicxml, to_musicxml};
use music21_rs::notation::StemDirection;
use music21_rs::{Stream, StreamElement};
use std::path::{Path, PathBuf};

mod musicxml_common;
use musicxml_common::{first_difference, normalize_ids, outline};

/// What is expected of a score.
#[derive(Clone, Copy)]
enum Expect {
    /// Both readers make the same score of it.
    Agrees,
    /// The outlines differ, for this reason.
    OutlineDiffers(&'static str),
    /// The outlines agree and the MusicXML differs, for this reason.
    MusicXmlDiffers(&'static str),
}

struct Score {
    name: &'static str,
    /// What it exercises.
    about: &'static str,
    /// Whether the style sheet MuseScore kept beside the file shows the name
    /// of its one part.
    style_shows_name: bool,
    expect: Expect,
}

const fn agrees(name: &'static str, about: &'static str) -> Score {
    Score {
        name,
        about,
        style_shows_name: false,
        expect: Expect::Agrees,
    }
}

const fn shown(name: &'static str, about: &'static str) -> Score {
    Score {
        name,
        about,
        style_shows_name: true,
        expect: Expect::Agrees,
    }
}

const SCORES: &[Score] = &[
    agrees("bwv66.6", "a four-part chorale with a pickup and fermatas"),
    agrees(
        "corelli-opus3no1-1grave",
        "a trio sonata: a brace and a bracket across parts, slurs, a title and a composer",
    ),
    agrees("mozart-k80-movement3", "a string quartet movement"),
    agrees("mozart-k545-exposition", "a piano piece on two staves"),
    shown(
        "two-staves",
        "a piano part on two staves with a slur and dynamics",
    ),
    shown("multiple-verses", "lyrics on several verses"),
    agrees("nested-tuplets", "tuplets inside tuplets"),
    shown("two-voices", "two voices in one staff"),
    shown("voices-with-chords", "chords inside voices"),
    shown(
        "foster-brown-hair",
        "a lead sheet: chord symbols, lyrics on two verses, a repeat with two endings",
    ),
    shown(
        "verdi-la-donna-e-mobile",
        "an aria: lyrics, hairpins, articulations, grace notes",
    ),
    agrees(
        "features",
        "a transposing clarinet and a piano: a grace note, a trill, a pedal line, a \
         hairpin, endings, tuplets with and without brackets, a tempo mark, and a key \
         and a clef that change",
    ),
    Score {
        name: "pickup",
        about: "a pickup measure one part rests through",
        style_shows_name: false,
        expect: Expect::OutlineDiffers(
            "MuseScore writes the rest that fills a pickup as a measure rest one beat long. \
             Its export says only that the rest fills its measure, and the MusicXML reader, \
             as music21 does, gives such a rest the whole bar of the meter, which pushes \
             every later measure of that part along. The file's length is the right one.",
        ),
    },
    Score {
        name: "arpeggios",
        about: "arpeggios on chords",
        style_shows_name: false,
        expect: Expect::MusicXmlDiffers(
            "MuseScore's export numbers every arpeggio 1, and a numbered arpeggio is one \
             drawn across several chords, so the MusicXML reader, as music21 does, joins \
             every arpeggio of the score into one. The file has an arpeggio on each chord.",
        ),
    },
    Score {
        name: "schoenberg-opus19-movement2",
        about: "a piano piece: a slur from one staff to the other, hairpins between notes",
        style_shows_name: false,
        expect: Expect::MusicXmlDiffers(
            "A slur starting on the lower staff ends on the upper one, which MuseScore's \
             export writes first, so a MusicXML reader meets its end before its start and \
             makes two half slurs of it. And a hairpin starting between two notes is \
             written after the note before it with an offset, so a MusicXML reader gives \
             it the next note read, which here is on the other staff. The file says where \
             each starts and ends, and the reader joins each to the notes under it.",
        ),
    },
];

fn data_directory() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/mscx")
}

/// Takes off the stems of the export's score that the file does not fix.
fn unfix_stems(file: &Stream, export: &mut Stream) {
    let fixed: Vec<Vec<bool>> = file
        .leaves()
        .iter()
        .map(|(_, element)| match element {
            StreamElement::Note(note) => vec![note.stem_direction() != StemDirection::Unspecified],
            StreamElement::Chord(chord) => chord
                .notes()
                .iter()
                .map(|note| note.stem_direction() != StemDirection::Unspecified)
                .collect(),
            _ => Vec::new(),
        })
        .collect();
    // Two scores that do not line up differ anyway, and say so below.
    if fixed.len() != export.leaves().len() {
        return;
    }
    let mut index = 0;
    export.for_each_mut(&mut |_, element| {
        let fixed = &fixed[index];
        index += 1;
        match element {
            StreamElement::Note(note) => {
                if fixed.first() == Some(&false) {
                    note.set_stem_direction(StemDirection::Unspecified);
                }
            }
            StreamElement::Chord(chord) => {
                for (note, fixed) in chord.notes_mut().iter_mut().zip(fixed) {
                    if !fixed {
                        note.set_stem_direction(StemDirection::Unspecified);
                    }
                }
            }
            _ => {}
        }
    });
}

/// A document with what the two sides are not compared on taken out.
fn comparable(document: &str, style_shows_name: bool) -> String {
    normalize_ids(document)
        .lines()
        .map(|line| {
            let trimmed = line.trim_start();
            if trimmed.starts_with("<grace") {
                line.replace(" slash=\"yes\"", "")
                    .replace(" slash=\"no\"", "")
            } else if trimmed.starts_with("<part-name") || trimmed.starts_with("<part-abbreviation")
            {
                let line = line.replace('\u{266d}', "b").replace('\u{266f}', "#");
                if style_shows_name {
                    line.replace(" print-object=\"no\"", "")
                } else {
                    line
                }
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// How a score came out.
enum Outcome {
    Agrees,
    OutlineDiffers(String),
    MusicXmlDiffers(String),
}

fn compare(directory: &Path, name: &str, style_shows_name: bool) -> Result<Outcome, String> {
    let read = |extension: &str| {
        let path = directory.join(format!("{name}.{extension}"));
        std::fs::read_to_string(&path).map_err(|error| format!("{}: {error}", path.display()))
    };
    let file =
        from_mscx(&read("mscx")?).map_err(|error| format!("the reader refused it: {error}"))?;
    let mut export = from_musicxml(&read("musicxml")?)
        .map_err(|error| format!("MuseScore's export could not be read: {error}"))?;

    let (ours, theirs) = (outline(&file), outline(&export));
    if let Some(difference) = first_difference(&ours, &theirs) {
        return Ok(Outcome::OutlineDiffers(difference));
    }
    unfix_stems(&file, &mut export);
    let options = ExportOptions::default();
    let write = |score: &Stream| {
        to_musicxml(score, &options)
            .map(|document| comparable(&document, style_shows_name))
            .map_err(|error| format!("it could not be written: {error}"))
    };
    let (ours, theirs) = (write(&file)?, write(&export)?);
    Ok(match first_difference(&ours, &theirs) {
        Some(difference) => {
            let kept = Path::new(env!("CARGO_MANIFEST_DIR")).join("../target/mscx-parity");
            if std::fs::create_dir_all(&kept).is_ok() {
                let _ = std::fs::write(kept.join(format!("{name}.mscx.musicxml")), &ours);
                let _ = std::fs::write(kept.join(format!("{name}.export.musicxml")), &theirs);
            }
            Outcome::MusicXmlDiffers(difference)
        }
        None => Outcome::Agrees,
    })
}

#[test]
fn the_crate_reads_a_musescore_file_as_musescore_exports_it() {
    let mut failures: Vec<String> = Vec::new();
    let mut agreeing = 0;
    let mut differing = 0;

    if let Ok(directory) = std::env::var("MSCX_PARITY_DIR") {
        let directory = PathBuf::from(directory);
        let mut names: Vec<String> = std::fs::read_dir(&directory)
            .expect("the directory named by MSCX_PARITY_DIR")
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| {
                path.extension()
                    .is_some_and(|extension| extension == "mscx")
            })
            .filter(|path| path.with_extension("musicxml").is_file())
            .filter_map(|path| {
                path.file_stem()
                    .map(|stem| stem.to_string_lossy().into_owned())
            })
            .collect();
        names.sort();
        for name in &names {
            match compare(&directory, name, false) {
                Ok(Outcome::Agrees) => agreeing += 1,
                Ok(Outcome::OutlineDiffers(difference)) => {
                    failures.push(format!("{name}: outline {difference}"));
                }
                Ok(Outcome::MusicXmlDiffers(difference)) => {
                    failures.push(format!("{name}: MusicXML {difference}"));
                }
                Err(error) => failures.push(format!("{name}: {error}")),
            }
        }
        println!("{agreeing} of {} scores agree", names.len());
        assert!(failures.is_empty(), "{}", failures.join("\n\n"));
        return;
    }

    let directory = data_directory();
    for score in SCORES {
        let Score { name, about, .. } = score;
        let outcome = match compare(&directory, name, score.style_shows_name) {
            Ok(outcome) => outcome,
            Err(error) => {
                failures.push(format!("{name} ({about}): {error}"));
                continue;
            }
        };
        match (score.expect, outcome) {
            (Expect::Agrees, Outcome::Agrees) => agreeing += 1,
            (Expect::OutlineDiffers(_), Outcome::OutlineDiffers(_))
            | (Expect::MusicXmlDiffers(_), Outcome::MusicXmlDiffers(_)) => differing += 1,
            (Expect::Agrees, Outcome::OutlineDiffers(difference)) => {
                failures.push(format!("{name} ({about}): outline {difference}"));
            }
            (Expect::Agrees, Outcome::MusicXmlDiffers(difference)) => {
                failures.push(format!("{name} ({about}): MusicXML {difference}"));
            }
            (Expect::OutlineDiffers(reason) | Expect::MusicXmlDiffers(reason), Outcome::Agrees) => {
                failures.push(format!(
                    "{name} ({about}) is listed as differing and now agrees; take it off \
                     the list. It was listed because: {reason}"
                ));
            }
            (Expect::OutlineDiffers(reason), Outcome::MusicXmlDiffers(difference)) => {
                failures.push(format!(
                    "{name} ({about}) is listed as differing in its outline, and its \
                     outline now agrees while its MusicXML differs: {difference}\n\
                     It was listed because: {reason}"
                ));
            }
            (Expect::MusicXmlDiffers(reason), Outcome::OutlineDiffers(difference)) => {
                failures.push(format!(
                    "{name} ({about}) is listed as agreeing in its outline, which now \
                     differs: {difference}\nIt was listed because: {reason}"
                ));
            }
        }
    }
    println!(
        "{agreeing} of {} scores agree, and {differing} differ as listed",
        SCORES.len()
    );
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

/// Every pair of files in the directory is in the list, so one added there
/// is compared.
#[test]
fn every_score_in_the_directory_is_listed() {
    let mut stems: Vec<String> = std::fs::read_dir(data_directory())
        .expect("the test scores")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "mscx")
        })
        .filter_map(|path| {
            path.file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
        })
        .collect();
    stems.sort();
    let mut listed: Vec<String> = SCORES.iter().map(|score| score.name.to_string()).collect();
    listed.sort();
    assert_eq!(stems, listed);
}
