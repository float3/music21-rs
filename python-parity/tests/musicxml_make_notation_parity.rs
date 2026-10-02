//! The crate's MusicXML writer with `make_notation` against music21's
//! exporter with `makeNotation=True`, which is music21's default.
//!
//! Each subject is read twice, by music21 and by the crate's own reader for
//! its format, and each side writes it with its notation made: music21's
//! `GeneralObjectExporter`, and `music21_rs::musicxml::to_musicxml` with
//! `ExportOptions::make_notation`. The two documents must be the same text.
//! The readers are held to music21's by their own tests, so what this
//! compares is what each side works out before it writes: measures, rests,
//! ties, accidentals, beams, tuplet brackets and the cutting of lengths no
//! single note value writes.
//!
//! The subjects are chiefly what music21 refuses to write with
//! `makeNotation=False`: MIDI files read into lengths no one value has, ABC
//! tunes read into no measures, and parts given no measures at all. Scores
//! that are already notated, from the corpus, are there to show that making
//! the notation of a finished score changes what music21 changes and no
//! more.
//!
//! music21 is music21 here, with nothing of the crate installed over it.
//!
//! A subject is named by its kind and a name: `tiny:` and a TinyNotation
//! line, `flat:` and one read as notes only, with no measures made, `loose:`
//! and notes each written `name@offset+length`, which may overlap, after a
//! meter if there is one, `scored:` and the same put in a score, which is
//! where music21 makes voices of what overlaps, `midi:`
//! and a path under the music21 package or `corpus:` and a corpus score
//! music21 first writes as MIDI, `abc:` and a corpus file with `#n` for one
//! tune of several, and `xml:` and a corpus score kept as MusicXML.
//! `MAKE_NOTATION_SUBJECTS`, a `;`-separated list of such names or `@` and
//! the path of a file of them, runs those instead of the ones below.
//!
//! Beside the subjects listed here the test runs every corpus score the
//! writer's own test lists that is kept as MusicXML, and every ABC tune of
//! the corpus that music21 reads into no measures.

use music21_rs::metadata::MetadataValue;
use music21_rs::musicxml::{ExportOptions, from_musicxml, to_musicxml};
use music21_rs::{Stream, StreamElement, StreamKind};
use music21_rs_python_parity::doctest::{add_dependency_venv, repo_root};
use pyo3::prelude::*;
use utils::{init_py, prepare};

mod musicxml_common;
use musicxml_common::{SCORES, STRIP_LAYOUT, first_difference, normalize_ids};

/// Subjects both sides notate alike, and what each exercises.
const SUBJECTS: &[(&str, &str)] = &[
    // TinyNotation lines, read into measures as music21 reads them.
    (
        "tiny:4/4 c4 d8 e f#2 g-4. an16 b c'1",
        "lengths kept until the next, accidentals",
    ),
    (
        "tiny:3/4 E4 r f# g=lastG trip{b-8 a g} c'2.",
        "a rest, a name, a triplet",
    ),
    (
        "tiny:CC4 C c c' c'' GG#1",
        "octaves either side of middle C",
    ),
    (
        "tiny:c2~ c~ c4 d e1~ e",
        "ties, one running on from another",
    ),
    ("tiny:c4 d e f g a b c' d' e'", "no meter, so common time"),
    (
        "tiny:6/8 c8 d e f g a 3/4 b4 c' d' 2/4 e' f'",
        "meters changing",
    ),
    ("tiny:4/4 c1 d0 e4", "a whole bar under a fermata"),
    (
        "tiny:3/4 c2. quad{d4 e f g} a2.",
        "four in the time of three",
    ),
    ("tiny:4/4 c4_doe d_ray e_me r_shh", "lyrics"),
    (
        "tiny:2/4 trip{c8 d e} trip{f16 g a} b8 trip{c'4 r d'}",
        "triplets of several values, one with a rest",
    ),
    (
        "tiny:c4 hello d r4 3 e..",
        "what is no token, and dots with no number",
    ),
    ("tiny:4/4 c2 d1 e4 f", "a note running past its barline"),
    ("tiny:5/8 c8 d e f g a4. b4", "an uneven meter"),
    (
        "tiny:4/4 c(#)4 d(-) e# f--2",
        "editorial accidentals, which are not the note's",
    ),
    (
        "tiny:4/4 trip{c4 d e~} e2 f4 g",
        "a brace closing the tie before the triplet",
    ),
    // The same lines as parts of loose notes, with no measures made.
    (
        "flat:4/4 c4 d8 e f#2 g-4. an16 b c'1",
        "lengths kept until the next, accidentals",
    ),
    (
        "flat:3/4 E4 r f# g=lastG trip{b-8 a g} c'2.",
        "a rest, a name, a triplet",
    ),
    (
        "flat:CC4 C c c' c'' GG#1",
        "octaves either side of middle C",
    ),
    (
        "flat:c2~ c~ c4 d e1~ e",
        "ties, one running on from another",
    ),
    ("flat:c4 d e f g a b c' d' e'", "no meter, so common time"),
    (
        "flat:6/8 c8 d e f g a 3/4 b4 c' d' 2/4 e' f'",
        "meters changing",
    ),
    ("flat:4/4 c1 d0 e4", "a whole bar under a fermata"),
    (
        "flat:3/4 c2. quad{d4 e f g} a2.",
        "four in the time of three",
    ),
    ("flat:4/4 c4_doe d_ray e_me r_shh", "lyrics"),
    (
        "flat:2/4 trip{c8 d e} trip{f16 g a} b8 trip{c'4 r d'}",
        "triplets of several values, one with a rest",
    ),
    (
        "flat:c4 hello d r4 3 e..",
        "what is no token, and dots with no number",
    ),
    ("flat:4/4 c2 d1 e4 f", "a note running past its barline"),
    ("flat:5/8 c8 d e f g a4. b4", "an uneven meter"),
    (
        "flat:4/4 c(#)4 d(-) e# f--2",
        "editorial accidentals, which are not the note's",
    ),
    (
        "flat:4/4 trip{c4 d e~} e2 f4 g",
        "a brace closing the tie before the triplet",
    ),
    // Notes placed one by one, as a part and as a score of one part.
    (
        "loose:3/4 C4@0+1 F#4@1+1 F4@2+1 G4@3+5",
        "a note across two barlines",
    ),
    (
        "loose:C4@0+1 E4@2+1 G4@5+1",
        "gaps, filled with rests that are not printed",
    ),
    (
        "loose:C4@0+0.5 D4@0.5+0.5 E4@1+1.25 F4@2.25+0.75 G4@6+2.5",
        "lengths no one value writes",
    ),
    (
        "loose:C4@0+4 E4@1+4 G4@2+4 B4@8+1",
        "notes that overlap, left in one line",
    ),
    (
        "scored:C4@0+2 E4@1+2 G4@3+1",
        "two notes that overlap, put in voices",
    ),
    ("scored:C4@0+4 E4@1+4 G4@2+4 B4@8+1", "three voices"),
    (
        "scored:3/4 C4@0+2 E4@1+1 G4@1.5+3 A4@7+1",
        "voices tied across a barline",
    ),
    (
        "scored:6/8 C4@0+1.5 E4@0.5+0.5 G4@1+0.5 A4@1.5+3 B4@3+0.5 C5@5+2",
        "voices in compound time",
    ),
    (
        "scored:C4@1+1 E4@1.5+1 G4@6+3 B4@7+0.25",
        "voices starting late",
    ),
    // MIDI files, most of which hold lengths no one note value has.
    ("midi:midi/testPrimitive/test01.mid", "one line of notes"),
    ("midi:midi/testPrimitive/test02.mid", "four parts"),
    (
        "midi:midi/testPrimitive/test03.mid",
        "overlapping notes put in voices",
    ),
    (
        "midi:midi/testPrimitive/test04.mid",
        "a long ensemble score",
    ),
    (
        "midi:midi/testPrimitive/test05.mid",
        "lengths no one value has",
    ),
    ("midi:midi/testPrimitive/test06.mid", "a tune in 6/8"),
    (
        "midi:midi/testPrimitive/test07.mid",
        "a long tune with ties",
    ),
    (
        "midi:midi/testPrimitive/test08.mid",
        "an instrument named by the track",
    ),
    ("midi:midi/testPrimitive/test09.mid", "two hands"),
    ("midi:midi/testPrimitive/test10.mid", "rests between notes"),
    (
        "midi:midi/testPrimitive/test11.mid",
        "three parts and a conductor track",
    ),
    (
        "midi:midi/testPrimitive/test12.mid",
        "four parts of whole notes",
    ),
    ("midi:midi/testPrimitive/test13.mid", "chords"),
    (
        "midi:midi/testPrimitive/test14.mid",
        "a chord running past a barline",
    ),
    ("midi:midi/testPrimitive/test15.mid", "a change of meter"),
    ("midi:midi/testPrimitive/test16.mid", "a change of tempo"),
    (
        "midi:midi/testPrimitive/test17.mid",
        "three parts with meters changing",
    ),
    ("midi:midi/testPrimitive/test18.mid", "lyrics"),
    (
        "midi:midi/testPrimitive/test19.mid",
        "lyrics, one hyphen alone",
    ),
    (
        "midi:midi/testPrimitive/test20.mid",
        "lyrics in another encoding",
    ),
    (
        "midi:midi/testPrimitive/test21.mid",
        "lyrics in another encoding again",
    ),
    ("midi:omr/k525MIDIMvt1.mid", "a string quartet movement"),
    ("midi:omr/k525short.mid", "its opening"),
    (
        "midi:corpus:bach/bwv66.6",
        "a chorale written as MIDI by music21",
    ),
    (
        "midi:corpus:schoenberg/opus19/movement6",
        "chords held under moving notes",
    ),
    (
        "midi:corpus:monteverdi/madrigal.3.1.rntxt",
        "chords with lyrics tied across barlines",
    ),
    (
        "midi:corpus:luca/gloria",
        "three voices in triple time, with triplets",
    ),
    (
        "midi:corpus:leadSheet/fosterBrownHair",
        "long notes cut where others already stand",
    ),
    (
        "midi:corpus:schumann_clara/polonaise_op1n2.mxl",
        "a piano piece in voices",
    ),
    // ABC tunes in measures.
    (
        "abc:ryansMammoth/7thRegimentReel.abc",
        "a reel with repeats",
    ),
    (
        "abc:ryansMammoth/42dHighlandRegimentStrathspey.abc",
        "measures holding more than their bar, cut in two",
    ),
    (
        "abc:ryansMammoth/BullDozerReel.abc",
        "a pickup of triplets, bowings and fingerings",
    ),
    (
        "abc:ryansMammoth/CzarOfRussiasFavoriteHornpipe.abc",
        "chords written highest note first, endings",
    ),
    (
        "abc:ryansMammoth/LafricansJig.abc",
        "a repeat moved onto the measure cut from its own",
    ),
    (
        "abc:ryansMammoth/RisingSunReel.abc",
        "an ending never closed",
    ),
    (
        "abc:oneills1850/0001-0050.abc#1",
        "one tune of a file of fifty",
    ),
    (
        "abc:oneills1850/0001-0050.abc#25",
        "a run of nine in the time of two",
    ),
    (
        "abc:oneills1850/1376-1475.abc#1427",
        "a grace note inside a triplet",
    ),
    (
        "abc:essenFolksong/han1.abc#10",
        "a folk song changing meter",
    ),
    ("abc:essenFolksong/erk20.abc#169", "a tie written on a rest"),
    ("abc:airdsAirs/book1.abc#5", "an air with a tempo"),
    (
        "abc:airdsAirs/book4.abc#0722",
        "barlines in the header, so the meter stands outside the measures",
    ),
    (
        "abc:essenFolksong/altdeu20.abc#148",
        "a meter given back to the measure after one cut in two",
    ),
    (
        "abc:essenFolksong/ballad20.abc#85",
        "a meter given back in 3/8",
    ),
    (
        "abc:essenFolksong/han1.abc#146",
        "a meter given back in 5/8",
    ),
    // Corpus scores a zero-length `<forward>` is read into as a quarter rest.
    (
        "xml:haydn/opus1no1/movement3.mxl",
        "rests running past the barline, cut and left unprinted",
    ),
    (
        "xml:schumann_robert/opus41no1/movement5.mxl",
        "the same in a long quartet movement",
    ),
];

/// Every tune of the corpus music21 reads into no measures, which its
/// exporter writes only by making them.
const UNMEASURED_TUNES: &[&str] = &[
    "essenFolksong/altdeu20.abc#113",
    "essenFolksong/ballad10.abc#26",
    "essenFolksong/ballad10.abc#27",
    "essenFolksong/ballad10.abc#28",
    "essenFolksong/ballad10.abc#85",
    "essenFolksong/ballad30.abc#132",
    "essenFolksong/ballad40.abc#137",
    "essenFolksong/ballad50.abc#12",
    "essenFolksong/ballad50.abc#16",
    "essenFolksong/ballad50.abc#18",
    "essenFolksong/ballad50.abc#109",
    "essenFolksong/ballad50.abc#111",
    "essenFolksong/ballad50.abc#116",
    "essenFolksong/ballad50.abc#117",
    "essenFolksong/ballad50.abc#119",
    "essenFolksong/ballad50.abc#185",
    "essenFolksong/ballad60.abc#2",
    "essenFolksong/ballad60.abc#83",
    "essenFolksong/dva0.abc#1",
    "essenFolksong/dva0.abc#9",
    "essenFolksong/dva0.abc#52",
    "essenFolksong/dva0.abc#67",
    "essenFolksong/erk10.abc#395",
    "essenFolksong/folkHaydn.abc#18",
    "essenFolksong/folkHaydn.abc#34",
    "essenFolksong/han1.abc#104",
    "essenFolksong/han1.abc#186",
    "essenFolksong/han1.abc#213",
    "essenFolksong/han1.abc#261",
    "essenFolksong/han1.abc#276",
    "essenFolksong/han1.abc#308",
    "essenFolksong/han1.abc#309",
    "essenFolksong/han1.abc#318",
    "essenFolksong/han1.abc#390",
    "essenFolksong/han1.abc#419",
    "essenFolksong/han1.abc#472",
    "essenFolksong/han1.abc#478",
    "essenFolksong/han2.abc#34",
    "essenFolksong/han2.abc#95",
    "essenFolksong/han2.abc#172",
    "essenFolksong/han2.abc#242",
    "essenFolksong/han2.abc#294",
    "essenFolksong/han2.abc#300",
    "essenFolksong/han2.abc#306",
    "essenFolksong/han2.abc#312",
    "essenFolksong/han2.abc#356",
    "essenFolksong/han2.abc#394",
    "essenFolksong/han2.abc#554",
    "essenFolksong/han2.abc#570",
    "essenFolksong/han2.abc#573",
    "essenFolksong/han2.abc#581",
    "essenFolksong/han2.abc#583",
    "essenFolksong/han2.abc#589",
    "essenFolksong/han2.abc#638",
    "essenFolksong/lot.abc#1",
    "essenFolksong/lot.abc#6",
    "essenFolksong/lot.abc#12",
    "essenFolksong/lot.abc#15",
    "essenFolksong/lot.abc#20",
    "essenFolksong/lot.abc#26",
    "essenFolksong/lot.abc#27",
    "essenFolksong/lot.abc#44",
    "essenFolksong/lot.abc#54",
    "essenFolksong/lot.abc#67",
    "essenFolksong/lot.abc#76",
    "essenFolksong/lot.abc#92",
    "essenFolksong/lot.abc#102",
    "essenFolksong/lot.abc#114",
    "essenFolksong/lot.abc#133",
    "essenFolksong/lot.abc#169",
    "essenFolksong/lot.abc#174",
    "essenFolksong/lot.abc#187",
    "essenFolksong/lot.abc#310",
    "essenFolksong/lot.abc#311",
    "essenFolksong/lot.abc#314",
    "essenFolksong/lot.abc#358",
    "essenFolksong/lot.abc#394",
    "essenFolksong/lot.abc#406",
    "essenFolksong/lux.abc#41",
    "essenFolksong/lux.abc#57",
    "essenFolksong/lux.abc#329",
    "essenFolksong/lux.abc#473",
    "essenFolksong/lux.abc#556",
    "essenFolksong/lux.abc#557",
    "essenFolksong/lux.abc#569",
    "essenFolksong/lux.abc#596",
    "essenFolksong/test0.abc#8",
    "essenFolksong/test0.abc#11",
    "essenFolksong/test0.abc#13",
    "essenFolksong/test0.abc#14",
    "essenFolksong/test0.abc#15",
    "essenFolksong/zuccal0.abc#17",
    "essenFolksong/zuccal0.abc#65",
    "essenFolksong/zuccal0.abc#129",
    "josquin/4vPerIlludAveProlatum.abc#1",
    "josquin/4vPerIlludAveProlatum.abc#2",
    "josquin/adieuMesAmours.abc#1",
    "josquin/adieuMesAmours.abc#2",
    "josquin/adieuMesAmours.abc#3",
    "josquin/adieuMesAmours.abc#4",
    "josquin/fortunaDunGranTempo.abc#1",
    "josquin/fortunaDunGranTempo.abc#2",
    "josquin/fortunaDunGranTempo.abc#3",
    "josquin/laPlusDesPlus.abc#1",
    "josquin/laPlusDesPlus.abc#2",
    "josquin/laPlusDesPlus.abc#3",
    "josquin/milleRegrets.abc#1",
    "josquin/milleRegrets.abc#2",
    "josquin/milleRegrets.abc#3",
    "josquin/milleRegrets.abc#4",
    "josquin/oVenusBant.abc#1",
    "josquin/oVenusBant.abc#2",
    "josquin/oVenusBant.abc#3",
    "josquin/petiteCamusette.abc#1",
    "josquin/petiteCamusette.abc#2",
    "josquin/petiteCamusette.abc#3",
    "josquin/petiteCamusette.abc#4",
    "josquin/petiteCamusette.abc#5",
];

/// How music21 reads each kind of subject, and the bytes of a corpus score
/// written as MIDI.
const HELPERS: &str = r#"
import os
import zipfile
from music21 import converter, corpus, midi, note, stream

def corpus_midi(name):
    # From the file itself: a cached score is a pickle, and one written
    # while this crate's classes stood in for music21's brings them back.
    score = corpus.parse(name, forceSource=True)
    return midi.translate.streamToMidiFile(score).writestr()

def flat_part(line):
    """A TinyNotation line's notes and rests, in a part with no measures."""
    read = converter.parse('tinyNotation: ' + line, makeNotation=False)
    part = stream.Part()
    for element in read.flatten():
        part.insert(element.getOffsetBySite(read.flatten()), element)
    return part

def loose_part(text):
    """A part of notes each standing where it is said to, for as long."""
    from music21 import meter
    part = stream.Part()
    for token in text.split():
        if '@' not in token:
            part.insert(0, meter.TimeSignature(token))
            continue
        name, placed = token.split('@')
        offset, length = placed.split('+')
        part.insert(float(offset), note.Note(name, quarterLength=float(length)))
    return part

def scored_part(text):
    score = stream.Score()
    score.insert(0, loose_part(text))
    return score

def source_text(name, musicxml=False):
    """The text of a corpus file, a compressed MusicXML one unpacked: None
    for a file in another format where MusicXML is asked for."""
    path = corpus.getWork(name)
    if isinstance(path, (list, tuple)):
        path = path[0]
    path = str(path)
    lower = path.lower()
    if lower.endswith('.mxl'):
        with zipfile.ZipFile(path) as archive:
            names = [n for n in archive.namelist()
                     if not n.startswith('META-INF') and n.lower().endswith(('.xml', '.musicxml'))]
            data = archive.read(names[0])
    elif musicxml and not lower.endswith(('.xml', '.musicxml')):
        return None
    else:
        with open(path, 'rb') as handle:
            data = handle.read()
    file_name = os.path.split(path)[1]
    for encoding in ('utf-8-sig', 'utf-16'):
        try:
            return data.decode(encoding), file_name
        except UnicodeDecodeError:
            pass
    return data.decode('latin-1'), file_name
"#;

#[test]
fn the_crate_makes_notation_as_music21_does() {
    let root = repo_root();
    std::env::set_current_dir(&root).expect("chdir to the repository root");
    prepare().expect("prepare the music21 reference checkout");

    let (failures, count) = Python::attach(|py| -> PyResult<(Vec<String>, usize)> {
        init_py(py)?;
        add_dependency_venv(py, &root)?;
        let _ = py.import("music21")?;
        let module = |code: &str, name: &str| -> PyResult<Bound<'_, PyModule>> {
            PyModule::from_code(
                py,
                &std::ffi::CString::new(code).expect("no nul in the helper"),
                &std::ffi::CString::new(format!("{name}.py")).expect("no nul in the name"),
                &std::ffi::CString::new(name).expect("no nul in the name"),
            )
        };
        let strip = module(STRIP_LAYOUT, "musicxml_parity_helpers")?;
        let helpers = module(HELPERS, "make_notation_helpers")?;
        let converter = py.import("music21.converter")?;
        let corpus = py.import("music21.corpus")?;
        let exporter = py.import("music21.musicxml.m21ToXml")?;
        let today: String = py
            .import("datetime")?
            .getattr("date")?
            .call_method0("today")?
            .str()?
            .extract()?;
        let version: String = py.import("music21")?.getattr("__version__")?.extract()?;
        let software = format!("music21 v.{version}");

        let chosen: Vec<String> = match std::env::var("MAKE_NOTATION_SUBJECTS") {
            Ok(list) => list
                .strip_prefix('@')
                .map(|path| std::fs::read_to_string(path).expect("the list of subjects"))
                .unwrap_or(list)
                .split([';', '\n'])
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .map(str::to_string)
                .collect(),
            Err(_) => SUBJECTS
                .iter()
                .map(|(name, _)| name.to_string())
                .chain(UNMEASURED_TUNES.iter().map(|tune| format!("abc:{tune}")))
                .chain(
                    SCORES
                        .iter()
                        .filter(|(name, _)| !name.starts_with("built:"))
                        .map(|(name, _)| format!("xml:{name}")),
                )
                .collect(),
        };
        // music21's converter, not its readers, signs what it reads.
        let signed = |mut score: Stream| {
            let mut metadata = score.metadata().cloned().unwrap_or_default();
            let mut names = vec![MetadataValue::new(software.clone())];
            names.extend(metadata.get("software").iter().cloned());
            metadata.set("software", names);
            score.set_metadata(Some(metadata));
            score
        };
        let mut failures = Vec::new();
        let mut agreed = 0;
        let mut compared = 0;
        for subject in &chosen {
            let Some((kind, name)) = subject.split_once(':') else {
                failures.push(format!("{subject}: no kind given"));
                continue;
            };
            let (theirs, ours) = match kind {
                "tiny" => (
                    converter.call_method1("parse", (format!("tinyNotation: {name}"),)),
                    music21_rs::tinynotation::from_tiny_notation(name),
                ),
                "flat" => (
                    helpers.getattr("flat_part")?.call1((name,)),
                    music21_rs::tinynotation::from_tiny_notation(name).map(|part| flattened(&part)),
                ),
                "loose" => (
                    helpers.getattr("loose_part")?.call1((name,)),
                    loose_part(name),
                ),
                "scored" => (
                    helpers.getattr("scored_part")?.call1((name,)),
                    loose_part(name).map(|part| {
                        let mut score = Stream::with_kind(StreamKind::Score);
                        score.insert(0.0, part);
                        score
                    }),
                ),
                "midi" => {
                    let bytes: Vec<u8> = match name.strip_prefix("corpus:") {
                        Some(work) => match helpers.getattr("corpus_midi")?.call1((work,)) {
                            Ok(bytes) => bytes.extract()?,
                            Err(error) => {
                                failures.push(format!(
                                    "{subject}: music21 could not write it as MIDI: {error}"
                                ));
                                continue;
                            }
                        },
                        None => std::fs::read(root.join("music21").join("music21").join(name))
                            .unwrap_or_else(|error| panic!("reading {name}: {error}")),
                    };
                    let kwargs = pyo3::types::PyDict::new(py);
                    kwargs.set_item("format", "midi")?;
                    (
                        converter.call_method(
                            "parseData",
                            (pyo3::types::PyBytes::new(py, &bytes),),
                            Some(&kwargs),
                        ),
                        music21_rs::midi::from_midi(&bytes),
                    )
                }
                "abc" => {
                    let (file, number) = match name.split_once('#') {
                        Some((file, number)) => (file, number.parse::<i32>().ok()),
                        None => (name, None),
                    };
                    let (text, _): (String, String) =
                        helpers.getattr("source_text")?.call1((file,))?.extract()?;
                    let kwargs = pyo3::types::PyDict::new(py);
                    kwargs.set_item("forceSource", true)?;
                    if let Some(number) = number {
                        kwargs.set_item("number", number)?;
                    }
                    let read = match number {
                        Some(number) => music21_rs::abc::from_abc_number(&text, number),
                        None => music21_rs::abc::from_abc(&text),
                    };
                    (
                        corpus.call_method("parse", (file,), Some(&kwargs)),
                        read.map(&signed),
                    )
                }
                "xml" => {
                    let source: Option<(String, String)> = helpers
                        .getattr("source_text")?
                        .call1((name, true))?
                        .extract()?;
                    let Some((text, file_name)) = source else {
                        // Kept in another format, which another reader reads.
                        continue;
                    };
                    let kwargs = pyo3::types::PyDict::new(py);
                    kwargs.set_item("forceSource", true)?;
                    (
                        corpus.call_method("parse", (name,), Some(&kwargs)),
                        from_musicxml(&text).map(|score| {
                            let mut score = signed(score);
                            let mut metadata = score.metadata().cloned().unwrap_or_default();
                            // A score with no title is called after its file.
                            if metadata.get("movementName").is_empty() {
                                metadata.add_text("movementName", file_name);
                            }
                            score.set_metadata(Some(metadata));
                            score
                        }),
                    )
                }
                other => {
                    failures.push(format!("{subject}: no subject is of the kind {other}"));
                    continue;
                }
            };
            compared += 1;
            let theirs = theirs
                .and_then(|read| strip.getattr("strip_layout")?.call1((read,)))
                .and_then(|read| {
                    exporter
                        .getattr("GeneralObjectExporter")?
                        .call1((&read,))?
                        .call_method0("parse")?
                        .call_method1("decode", ("utf-8",))?
                        .extract::<String>()
                });
            let theirs = match theirs {
                Ok(text) => text,
                Err(error) => {
                    failures.push(format!(
                        "{subject}: music21 could not read or write it: {error}"
                    ));
                    continue;
                }
            };
            let ours = match ours {
                Ok(stream) => stream,
                Err(error) => {
                    failures.push(format!("{subject}: the crate could not read it: {error}"));
                    continue;
                }
            };
            let options = ExportOptions {
                encoding_date: Some(today.clone()),
                software: software.clone(),
                make_notation: true,
                ..ExportOptions::default()
            };
            match to_musicxml(&ours, &options) {
                Ok(ours) => {
                    // The layout strip takes the weight off a tempo word on
                    // music21's side, where the crate's writer always
                    // writes it.
                    let ours = normalize_ids(&ours).replace(
                        "<words default-y=\"45\" font-weight=\"bold\">",
                        "<words default-y=\"45\">",
                    );
                    let theirs = normalize_ids(theirs.trim_end());
                    match first_difference(&ours, &theirs) {
                        Some(difference) => {
                            let directory = root.join("target").join("make-notation-parity");
                            let stem: String = subject
                                .chars()
                                .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
                                .take(80)
                                .collect();
                            if std::fs::create_dir_all(&directory).is_ok() {
                                let _ = std::fs::write(
                                    directory.join(format!("{stem}.music21.xml")),
                                    &theirs,
                                );
                                let _ = std::fs::write(
                                    directory.join(format!("{stem}.music21-rs.xml")),
                                    &ours,
                                );
                            }
                            failures.push(format!("{subject}: {difference}"));
                        }
                        None => agreed += 1,
                    }
                }
                Err(error) => failures.push(format!("{subject}: the crate refused it: {error}")),
            }
        }
        println!("{agreed} of {compared} agree");
        Ok((failures, compared))
    })
    .unwrap_or_else(|error| panic!("running music21: {error}"));

    assert!(
        failures.is_empty(),
        "{} of {} subjects are notated differently:\n\n{}",
        failures.len(),
        count,
        failures.join("\n\n")
    );
}

/// A part of notes each standing where it is said to, for as long, as the
/// helper builds music21's.
fn loose_part(text: &str) -> music21_rs::Result<Stream> {
    let mut part = Stream::with_kind(StreamKind::Part);
    for token in text.split_whitespace() {
        let Some((name, placed)) = token.split_once('@') else {
            part.insert(0.0, music21_rs::TimeSignature::from_ratio_string(token)?);
            continue;
        };
        let (offset, length) = placed.split_once('+').unwrap_or((placed, "1"));
        let number = |text: &str| text.parse::<f64>().unwrap_or(0.0);
        let note = music21_rs::Note::from_name(name)?
            .with_duration(music21_rs::Duration::new(number(length))?);
        part.insert(number(offset), note);
    }
    Ok(part)
}

/// A part's notes and rests with its measures taken away, as the helper
/// reads music21's.
fn flattened(part: &Stream) -> Stream {
    let mut flat = Stream::with_kind(StreamKind::Part);
    for event in part.flatten().events() {
        if matches!(event.element(), StreamElement::Stream(_)) {
            continue;
        }
        flat.insert(event.offset(), event.element().clone());
    }
    flat
}
