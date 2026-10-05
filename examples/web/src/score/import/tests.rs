//! Every reader against small files built here in each format, and the
//! writer against what they read.

use super::abc::{rests, to_abc};
use super::*;

fn abc_of(name: &str, bytes: &[u8]) -> String {
    let imported = read_file(name, bytes).expect("the file reads");
    to_abc(&imported).expect("the score writes")
}

/// The music of the first voice, every field and directive left out.
fn music(abc: &str) -> String {
    voice_music(abc, 1)
}

/// The music of the `n`th voice, counted from one: what follows its `V:`
/// line, or the whole tune where it has no `V:` lines, fields and
/// directives left out.
fn voice_music(abc: &str, n: usize) -> String {
    let mut voices = 0;
    let mut lines = Vec::new();
    for line in abc.lines() {
        if line.starts_with("V:") {
            voices += 1;
        }
        let field = line.starts_with("%%")
            || line.len() > 1
                && line.as_bytes()[1] == b':'
                && line.as_bytes()[0].is_ascii_alphabetic();
        if !field && voices.max(1) == n {
            lines.push(line.trim());
        }
    }
    lines.join(" ")
}

// --------------------------------------------------------------- Songsterr

const SONGSTERR: &str = r#"{
  "name": "Lead", "instrument": "Acoustic Guitar (nylon)", "instrumentId": 24,
  "strings": 6, "tuning": [64, 59, 55, 50, 45, 40],
  "automations": {"tempo": [{"measure": 0, "position": 0, "bpm": 96, "type": 4}]},
  "measures": [
    {"signature": [3, 4], "keySignature": {"accidentalCount": 1, "mode": "major", "transposeAs": "b"},
     "repeatStart": true, "marker": {"text": "Intro"},
     "voices": [{"beats": [
       {"type": 4, "duration": [1, 4], "notes": [{"string": 1, "fret": 1}], "chord": {"text": "C"}},
       {"type": 8, "duration": [1, 12], "tuplet": 3, "tupletStart": true, "notes": [{"string": 0, "fret": 0}]},
       {"type": 8, "duration": [1, 12], "tuplet": 3, "notes": [{"string": 0, "fret": 1}]},
       {"type": 8, "duration": [1, 12], "tuplet": 3, "tupletStop": true, "notes": [{"string": 0, "fret": 3}]},
       {"type": 4, "duration": [1, 4], "notes": [{"string": 2, "fret": 2}]}
     ]}]},
    {"repeat": 2, "alternateEnding": [1],
     "voices": [{"beats": [
       {"type": 4, "duration": [1, 4], "notes": [{"string": 2, "fret": 2, "tie": true}]},
       {"type": 4, "duration": [3, 8], "dots": 1, "notes": [{"string": 3, "fret": 2}, {"string": 2, "fret": 0}]},
       {"type": 8, "duration": [1, 8], "rest": true, "notes": [{"rest": true}]}
     ]}]},
    {"alternateEnding": [2],
     "voices": [{"beats": [
       {"type": 32, "duration": [1, 32], "graceNote": "beforeBeat", "notes": [{"string": 1, "fret": 3}]},
       {"type": 2, "duration": [3, 4], "dots": 1, "notes": [{"string": 1, "fret": 1}, {"string": 5, "fret": 3, "dead": true}]}
     ]}]}
  ]
}"#;

#[test]
fn a_songsterr_track_keeps_its_repeats_tuplets_ties_and_chords() {
    let abc = abc_of("0.json", SONGSTERR.as_bytes());
    assert!(abc.contains("M:3/4"), "{abc}");
    assert!(abc.contains("Q:1/4=96"), "{abc}");
    assert!(abc.contains("K:F"), "one flat, major: {abc}");
    assert!(
        abc.contains("V:1 clef=treble-8 name=\"Lead\"\n%%MIDI program 24"),
        "{abc}"
    );
    // String 1 is the B string: its first fret is C4.
    assert_eq!(
        music(&abc),
        "|: \"C\"\"^Intro\"C2 (3EFG A,2- |1 A,2 [E,G,]3 z :|2 {D/4}C6 |]"
    );
}

#[test]
fn a_songsterr_drum_track_is_left_out_with_the_reason() {
    let drums = r#"{"name": "Kit", "instrumentId": 1024, "tuning": [], "measures": [
        {"voices": [{"beats": [{"type": 4, "duration": [1, 4], "notes": [{"string": 1, "fret": 38}]}]}]}]}"#;
    let imported = read_file("1.json", drums.as_bytes()).expect("the file reads");
    assert!(imported.parts.is_empty());
    assert_eq!(imported.skipped, ["Kit: drums"]);
    let error = to_abc(&imported).unwrap_err();
    assert!(error.contains("Kit: drums"), "{error}");
}

#[test]
fn songsterr_parts_added_together_make_one_score() {
    let mut import = ScoreImport::new();
    import
        .add("0.json", SONGSTERR.as_bytes())
        .expect("the first part reads");
    import
        .add(
            "1.json",
            SONGSTERR.replace("\"Lead\"", "\"Rhythm\"").as_bytes(),
        )
        .expect("the second part reads");
    let abc = to_abc(&import.imported).expect("the score writes");
    assert!(abc.contains("V:2 clef=treble-8 name=\"Rhythm\""), "{abc}");
    // Each part's music, bar for bar.
    let bars = |n: usize| voice_music(&abc, n).matches('|').count();
    assert_eq!(bars(2), bars(1));
}

// ---------------------------------------------------------------- MusicXML

const MUSICXML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE score-partwise PUBLIC "-//Recordare//DTD MusicXML 4.0 Partwise//EN" "http://www.musicxml.org/dtds/partwise.dtd">
<score-partwise version="4.0">
  <work><work-title>Little Tune</work-title></work>
  <part-list>
    <score-part id="P1"><part-name>Clarinet in B-flat</part-name>
      <midi-instrument id="P1-I1"><midi-channel>1</midi-channel><midi-program>72</midi-program></midi-instrument>
    </score-part>
  </part-list>
  <part id="P1">
    <measure number="0" implicit="yes">
      <attributes>
        <divisions>6</divisions>
        <key><fifths>2</fifths><mode>major</mode></key>
        <time><beats>4</beats><beat-type>4</beat-type></time>
        <clef><sign>G</sign><line>2</line></clef>
        <transpose><diatonic>-1</diatonic><chromatic>-2</chromatic></transpose>
      </attributes>
      <direction><sound tempo="72"/></direction>
      <note><pitch><step>A</step><octave>4</octave></pitch><duration>6</duration><voice>1</voice><type>quarter</type></note>
    </measure>
    <measure number="1">
      <barline location="left"><repeat direction="forward"/></barline>
      <harmony><root><root-step>D</root-step></root><kind text="">major</kind></harmony>
      <note><pitch><step>D</step><octave>5</octave></pitch><duration>12</duration><tie type="start"/><voice>1</voice><type>half</type></note>
      <note><pitch><step>D</step><octave>5</octave></pitch><duration>6</duration><tie type="stop"/><voice>1</voice><type>quarter</type></note>
      <note><pitch><step>F</step><alter>1</alter><octave>4</octave></pitch><duration>6</duration><voice>1</voice><type>quarter</type></note>
      <note><chord/><pitch><step>A</step><octave>4</octave></pitch><duration>6</duration><voice>1</voice><type>quarter</type></note>
      <backup><duration>24</duration></backup>
      <note><pitch><step>D</step><octave>4</octave></pitch><duration>24</duration><voice>2</voice><type>whole</type></note>
      <barline location="right"><repeat direction="backward" times="3"/></barline>
    </measure>
    <measure number="2">
      <note><pitch><step>E</step><octave>5</octave></pitch><duration>2</duration><voice>1</voice><type>eighth</type><time-modification><actual-notes>3</actual-notes><normal-notes>2</normal-notes></time-modification></note>
      <note><pitch><step>D</step><octave>5</octave></pitch><duration>2</duration><voice>1</voice><type>eighth</type><time-modification><actual-notes>3</actual-notes><normal-notes>2</normal-notes></time-modification></note>
      <note><pitch><step>C</step><alter>1</alter><octave>5</octave></pitch><duration>2</duration><voice>1</voice><type>eighth</type><time-modification><actual-notes>3</actual-notes><normal-notes>2</normal-notes></time-modification></note>
      <note><rest/><duration>6</duration><voice>1</voice><type>quarter</type></note>
      <note><pitch><step>D</step><octave>5</octave></pitch><duration>9</duration><voice>1</voice><type>quarter</type><dot/></note>
    </measure>
  </part>
</score-partwise>"#;

#[test]
fn musicxml_sounds_at_concert_pitch_and_keeps_its_voices() {
    let abc = abc_of("tune.musicxml", MUSICXML.as_bytes());
    assert!(abc.contains("T:Little Tune"), "{abc}");
    assert!(abc.contains("Q:1/4=72"), "{abc}");
    // Written in D for a B-flat clarinet, it sounds in C.
    assert!(abc.contains("K:C\n"), "{abc}");
    assert!(abc.contains("%%score (1 2)"), "{abc}");
    assert!(abc.contains("%%MIDI program 71"), "{abc}");
    assert_eq!(
        music(&abc),
        "G2 |: \"D\"\"^×3\"c4- c2 [EG]2 :| (3dc B z2 c3 |]"
    );
    // The lower voice is filled out with rests nothing draws.
    assert_eq!(voice_music(&abc, 2), "x2 |: C8 :| x4 x2 x |]");
}

/// A guitar part on a staff and a tablature staff under it, and a drum part.
const GUITAR: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<score-partwise version="4.0">
  <part-list>
    <score-part id="P1"><part-name>Guitar</part-name></score-part>
    <score-part id="P2"><part-name>Kit</part-name>
      <midi-instrument id="P2-I1"><midi-channel>10</midi-channel></midi-instrument>
    </score-part>
  </part-list>
  <part id="P1">
    <measure number="1">
      <attributes>
        <divisions>1</divisions>
        <time><beats>2</beats><beat-type>4</beat-type></time>
        <staves>2</staves>
        <clef number="1"><sign>G</sign><line>2</line><clef-octave-change>-1</clef-octave-change></clef>
        <clef number="2"><sign>TAB</sign><line>5</line></clef>
        <staff-details number="2"><staff-lines>6</staff-lines>
          <staff-tuning line="1"><tuning-step>E</tuning-step><tuning-octave>2</tuning-octave></staff-tuning>
          <staff-tuning line="2"><tuning-step>A</tuning-step><tuning-octave>2</tuning-octave></staff-tuning>
          <staff-tuning line="3"><tuning-step>D</tuning-step><tuning-octave>3</tuning-octave></staff-tuning>
        </staff-details>
      </attributes>
      <direction><direction-type><rehearsal>Intro</rehearsal></direction-type></direction>
      <note><pitch><step>E</step><octave>3</octave></pitch><duration>2</duration><voice>1</voice><type>half</type><staff>1</staff></note>
      <backup><duration>2</duration></backup>
      <note><pitch><step>E</step><octave>3</octave></pitch><duration>2</duration><voice>2</voice><type>half</type><staff>2</staff></note>
    </measure>
  </part>
  <part id="P2">
    <measure number="1">
      <attributes><divisions>1</divisions></attributes>
      <note><unpitched><display-step>C</display-step><display-octave>5</display-octave></unpitched><duration>2</duration><type>half</type></note>
    </measure>
  </part>
</score-partwise>"#;

#[test]
fn a_tablature_staff_doubling_its_notes_and_a_drum_part_are_left_out() {
    let imported = read_file("guitar.musicxml", GUITAR.as_bytes()).expect("the file reads");
    assert_eq!(imported.parts.len(), 1, "{:?}", imported.skipped);
    assert_eq!(imported.parts[0].name, "Guitar");
    assert_eq!(imported.parts[0].clef.as_deref(), Some("treble-8"));
    assert_eq!(
        imported.skipped,
        [
            "Guitar: its tablature staff, which doubles the notes",
            "Kit: drums"
        ]
    );
    assert_eq!(
        imported.parts[0].measures[0].marker.as_deref(),
        Some("Intro")
    );
    let abc = to_abc(&imported).expect("the score writes");
    assert!(abc.contains("\"^Intro\""), "{abc}");
}

#[test]
fn a_tablature_staff_alone_keeps_its_tuning() {
    // The same part with its notation staff taken away.
    let alone = GUITAR
        .replace("<staves>2</staves>", "")
        .replace(
            "<clef number=\"1\"><sign>G</sign><line>2</line><clef-octave-change>-1</clef-octave-change></clef>",
            "",
        )
        .replace("<clef number=\"2\">", "<clef>")
        .replace("<staff-details number=\"2\">", "<staff-details>")
        .replace("<staff>1</staff>", "")
        .replace("<staff>2</staff>", "");
    let imported = read_file("tab.musicxml", alone.as_bytes()).expect("the file reads");
    assert_eq!(imported.parts.len(), 1, "{:?}", imported.skipped);
    assert_eq!(imported.parts[0].tuning, [40, 45, 50]);
}

/// A zip archive of the given files, stored rather than deflated.
fn stored_zip(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut archive = Vec::new();
    let mut directory = Vec::new();
    for (name, data) in files {
        let offset = archive.len() as u32;
        let header = |signature: &[u8], central: bool| {
            let mut out = signature.to_vec();
            if central {
                out.extend([20, 0]);
            }
            out.extend([20, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
            out.extend((data.len() as u32).to_le_bytes());
            out.extend((data.len() as u32).to_le_bytes());
            out.extend((name.len() as u16).to_le_bytes());
            out.extend([0, 0]);
            if central {
                // Comment length, disk, and internal and external attributes.
                out.extend([0; 10]);
                out.extend(offset.to_le_bytes());
            }
            out.extend(name.as_bytes());
            out
        };
        archive.extend(header(b"PK\x03\x04", false));
        archive.extend(*data);
        directory.extend(header(b"PK\x01\x02", true));
    }
    let start = archive.len() as u32;
    archive.extend(&directory);
    archive.extend(b"PK\x05\x06\0\0\0\0");
    archive.extend((files.len() as u16).to_le_bytes());
    archive.extend((files.len() as u16).to_le_bytes());
    archive.extend((directory.len() as u32).to_le_bytes());
    archive.extend(start.to_le_bytes());
    archive.extend([0, 0]);
    archive
}

#[test]
fn compressed_musicxml_is_read_through_its_container() {
    let container = br#"<?xml version="1.0"?><container><rootfiles><rootfile full-path="score/tune.xml"/></rootfiles></container>"#;
    let archive = stored_zip(&[
        ("META-INF/container.xml", container),
        ("score/tune.xml", MUSICXML.as_bytes()),
    ]);
    assert_eq!(
        abc_of("tune.mxl", &archive),
        abc_of("tune.musicxml", MUSICXML.as_bytes())
    );
}

#[test]
fn a_deflated_zip_entry_inflates() {
    let text = MUSICXML.as_bytes();
    let packed = miniz_oxide::deflate::compress_to_vec(text, 6);
    let mut archive = stored_zip(&[("tune.xml", &packed)]);
    // Mark the one entry deflated, in its local header and in the directory.
    archive[8] = 8;
    let central = archive
        .windows(4)
        .position(|window| window == b"PK\x01\x02")
        .expect("a directory");
    archive[central + 10] = 8;
    let read = zip::Archive::new(&archive)
        .and_then(|zip| zip.read("tune.xml"))
        .expect("the entry reads");
    assert_eq!(read.as_deref(), Some(text));
}

#[test]
fn utf16_musicxml_is_decoded() {
    let mut bytes = vec![0xff, 0xfe];
    bytes.extend(MUSICXML.encode_utf16().flat_map(u16::to_le_bytes));
    assert_eq!(
        abc_of("tune.xml", &bytes),
        abc_of("tune.xml", MUSICXML.as_bytes())
    );
}

// ------------------------------------------------------ Guitar Pro 6 to 8

const GPIF: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<GPIF>
  <Score><Title><![CDATA[Riff]]></Title></Score>
  <MasterTrack><Automations><Automation><Type>Tempo</Type><Value>100 2</Value></Automation></Automations></MasterTrack>
  <Tracks>
    <Track id="0"><Name><![CDATA[Guitar]]></Name>
      <Staves><Staff><Properties>
        <Property name="Tuning"><Pitches>40 45 50 55 59 64</Pitches></Property>
        <Property name="DiagramCollection"><Items><Item id="0" name="Em"/></Items></Property>
      </Properties></Staff></Staves>
      <Sounds><Sound><MIDI><Program>29</Program></MIDI></Sound></Sounds>
    </Track>
    <Track id="1"><Name><![CDATA[Kit]]></Name><InstrumentSet><Type>drumKit</Type></InstrumentSet>
      <Staves><Staff><Properties/></Staff></Staves>
    </Track>
  </Tracks>
  <MasterBars>
    <MasterBar><Key><AccidentalCount>1</AccidentalCount><Mode>Minor</Mode></Key><Time>2/4</Time>
      <Repeat start="true" end="false" count="0"/><Section><Text><![CDATA[Riff]]></Text></Section><Bars>0 2</Bars></MasterBar>
    <MasterBar><Key><AccidentalCount>1</AccidentalCount><Mode>Minor</Mode></Key><Time>2/4</Time>
      <Repeat start="false" end="true" count="2"/><Bars>1 2</Bars></MasterBar>
  </MasterBars>
  <Bars>
    <Bar id="0"><Voices>0 -1 -1 -1</Voices></Bar>
    <Bar id="1"><Voices>1 -1 -1 -1</Voices></Bar>
    <Bar id="2"><Voices>-1 -1 -1 -1</Voices></Bar>
  </Bars>
  <Voices>
    <Voice id="0"><Beats>0 1 2</Beats></Voice>
    <Voice id="1"><Beats>3 4</Beats></Voice>
  </Voices>
  <Beats>
    <Beat id="0"><Rhythm ref="0"/><Chord>0</Chord><Notes>0</Notes></Beat>
    <Beat id="1"><Rhythm ref="1"/><GraceNotes>BeforeBeat</GraceNotes><Notes>1</Notes></Beat>
    <Beat id="2"><Rhythm ref="0"/><Notes>2</Notes></Beat>
    <Beat id="3"><Rhythm ref="0"/><Notes>3</Notes></Beat>
    <Beat id="4"><Rhythm ref="0"/></Beat>
  </Beats>
  <Notes>
    <Note id="0"><Properties><Property name="String"><String>0</String></Property><Property name="Fret"><Fret>0</Fret></Property></Properties></Note>
    <Note id="1"><Properties><Property name="String"><String>1</String></Property><Property name="Fret"><Fret>2</Fret></Property></Properties></Note>
    <Note id="2"><Tie origin="true" destination="false"/><Properties><Property name="Midi"><Number>57</Number></Property></Properties></Note>
    <Note id="3"><Tie origin="false" destination="true"/><Properties><Property name="Midi"><Number>57</Number></Property></Properties></Note>
  </Notes>
  <Rhythms>
    <Rhythm id="0"><NoteValue>Quarter</NoteValue></Rhythm>
    <Rhythm id="1"><NoteValue>32nd</NoteValue></Rhythm>
  </Rhythms>
</GPIF>"#;

const GPIF_MUSIC: &str = "|: \"Em\"\"^Riff\"E,,2 {B,,/4}A,2- | A,2 z2 :|";

#[test]
fn a_guitar_pro_7_file_reads_from_its_zip() {
    let archive = stored_zip(&[("VERSION", b"7.0"), ("Content/score.gpif", GPIF.as_bytes())]);
    let imported = read_file("riff.gp", &archive).expect("the file reads");
    assert_eq!(imported.skipped, ["Kit: drums"]);
    assert_eq!(imported.parts[0].tuning, [40, 45, 50, 55, 59, 64]);
    let abc = to_abc(&imported).expect("the score writes");
    assert!(abc.contains("T:Riff\nM:2/4"), "{abc}");
    assert!(abc.contains("Q:1/4=100"), "{abc}");
    assert!(abc.contains("K:Em"), "{abc}");
    assert!(
        abc.contains("V:1 clef=treble-8 name=\"Guitar\"\n%%MIDI program 29"),
        "{abc}"
    );
    assert_eq!(music(&abc), GPIF_MUSIC);
}

/// A Guitar Pro 6 file system holding `score.gpif`: a header sector, one
/// entry naming the data sectors, then the data.
fn gpx_file_system(score: &[u8]) -> Vec<u8> {
    const SECTOR: usize = 0x1000;
    let sectors = score.len().div_ceil(SECTOR);
    let mut system = vec![0; SECTOR * (2 + sectors)];
    let entry = SECTOR;
    system[entry..entry + 4].copy_from_slice(&2_i32.to_le_bytes());
    system[entry + 4..entry + 4 + 10].copy_from_slice(b"score.gpif");
    system[entry + 0x8c..entry + 0x90].copy_from_slice(&(score.len() as i32).to_le_bytes());
    for sector in 0..sectors {
        let pointer = entry + 0x94 + 4 * sector;
        system[pointer..pointer + 4].copy_from_slice(&((2 + sector) as i32).to_le_bytes());
        let chunk = &score[sector * SECTOR..((sector + 1) * SECTOR).min(score.len())];
        system[(2 + sector) * SECTOR..(2 + sector) * SECTOR + chunk.len()].copy_from_slice(chunk);
    }
    let mut file = b"BCFS".to_vec();
    file.extend(system);
    file
}

/// Guitar Pro 6's compression, written with literal runs only.
fn bcfz(data: &[u8]) -> Vec<u8> {
    let mut bits: Vec<u8> = Vec::new();
    for run in data.chunks(3) {
        bits.push(0);
        bits.extend((0..2).map(|index| (run.len() >> index) as u8 & 1));
        for byte in run {
            bits.extend((0..8).rev().map(|index| byte >> index & 1));
        }
    }
    let mut out = b"BCFZ".to_vec();
    out.extend((data.len() as i32).to_le_bytes());
    out.extend(bits.chunks(8).map(|byte| {
        byte.iter()
            .enumerate()
            .fold(0, |value, (index, bit)| value | bit << (7 - index))
    }));
    out
}

#[test]
fn a_guitar_pro_6_file_reads_whether_or_not_it_is_compressed() {
    let plain = gpx_file_system(GPIF.as_bytes());
    assert_eq!(music(&abc_of("riff.gpx", &plain)), GPIF_MUSIC);
    assert_eq!(music(&abc_of("riff.gpx", &bcfz(&plain))), GPIF_MUSIC);
}

// ------------------------------------------------------ Guitar Pro 3 to 5

/// A Guitar Pro 3 file of one six-string track, from measure headers
/// (flags and their data) and beats (flags and their data) written out.
fn gp3(headers: &[&[u8]], beats: &[&[&[u8]]]) -> Vec<u8> {
    let int = |value: i32| value.to_le_bytes().to_vec();
    let text = |value: &str| {
        let mut out = int(value.len() as i32 + 1);
        out.push(value.len() as u8);
        out.extend(value.as_bytes());
        out
    };
    let banner = b"FICHIER GUITAR PRO v3.00";
    let mut file = vec![banner.len() as u8];
    file.extend(banner);
    file.extend([0; 6]);
    file.extend(text("Etude"));
    for _ in 0..7 {
        file.extend(text(""));
    }
    file.extend(int(0)); // notices
    file.push(0); // triplet feel
    file.extend(int(88));
    file.extend(int(0)); // key
    for _ in 0..64 {
        file.extend(int(24));
        file.extend([0; 8]);
    }
    file.extend(int(headers.len() as i32));
    file.extend(int(1));
    for header in headers {
        file.extend(*header);
    }
    file.push(0); // track flags
    file.push(6);
    let mut name = b"Nylon".to_vec();
    name.resize(40, 0);
    file.extend(name);
    file.extend(int(6));
    for open in [64, 59, 55, 50, 45, 40, -1] {
        file.extend(int(open));
    }
    for value in [1, 1, 2, 24, 0] {
        file.extend(int(value));
    }
    file.extend([0; 4]); // colour
    for measure in beats {
        file.extend(int(measure.len() as i32));
        for beat in *measure {
            file.extend(*beat);
        }
    }
    file
}

#[test]
fn a_guitar_pro_3_file_reads_its_headers_notes_and_ties() {
    // Bar one opens a repeat in 2/4 with a marker; bar two closes it, played
    // three times.
    let mut marked = vec![0x01 | 0x02 | 0x04 | 0x20, 2, 4];
    marked.extend(6_i32.to_le_bytes());
    marked.push(5);
    marked.extend(b"Verse");
    marked.extend([0; 4]);
    let closing = [0x08_u8, 2];
    let file = gp3(
        &[&marked, &closing],
        &[
            &[
                // A quarter on the G string, third fret, then a triplet
                // eighth on the B string, open.
                &[0x00, 0, 1 << 4, 0x20, 1, 3],
                &[0x20, 1, 3, 0, 0, 0, 1 << 5, 0x20, 1, 0],
                &[0x20, 1, 3, 0, 0, 0, 1 << 5, 0x20, 1, 0],
                &[0x20, 1, 3, 0, 0, 0, 1 << 5, 0x20, 1, 0],
            ],
            &[
                // The B string held over, then a dotted quarter rest.
                &[0x00, 0, 1 << 5, 0x20, 2, 0],
                &[0x41, 2, 0, 0],
            ],
        ],
    );
    let abc = abc_of("etude.gp3", &file);
    assert!(abc.contains("T:Etude\nM:2/4"), "{abc}");
    assert!(abc.contains("Q:1/4=88"), "{abc}");
    assert!(abc.contains("%%MIDI program 24"), "{abc}");
    assert_eq!(
        music(&abc),
        "|: \"^Verse\"^A,2 (3B,B,B,- | \"^×3\"B,2 z3 :|"
    );
}

#[test]
fn a_file_of_no_known_kind_is_refused_by_name() {
    let error = read_file("notes.txt", b"just words").unwrap_err();
    assert!(error.contains("notes.txt"), "{error}");
}

// ------------------------------------------------------------------ writer

#[test]
fn a_short_bar_in_the_middle_is_filled_but_a_pickup_is_not() {
    let beat = |ticks| Beat {
        written: ticks,
        notes: vec![Note {
            midi: 60,
            tied: false,
        }],
        ..Beat::default()
    };
    let measure = |beats: Vec<Beat>| Measure {
        meter: Some((4, 4)),
        voices: vec![beats],
        ..Measure::default()
    };
    let imported = Imported {
        parts: vec![Part {
            name: "Line".to_string(),
            measures: vec![
                measure(vec![beat(TICKS)]),
                measure(vec![beat(TICKS * 2)]),
                measure(vec![beat(TICKS * 4)]),
            ],
            ..Part::default()
        }],
        ..Imported::default()
    };
    let abc = to_abc(&imported).expect("the score writes");
    assert_eq!(music(&abc), "C2 | C4 z4 | C8 |]");
}

#[test]
fn every_tuplet_number_has_a_ratio() {
    assert_eq!(tuplet_ratio(3), Some((3, 2)));
    assert_eq!(tuplet_ratio(5), Some((5, 4)));
    assert_eq!(tuplet_ratio(6), Some((6, 4)));
    assert_eq!(tuplet_ratio(7), Some((7, 4)));
    assert_eq!(tuplet_ratio(9), Some((9, 8)));
    assert_eq!(tuplet_ratio(2), Some((2, 3)));
    assert_eq!(tuplet_ratio(1), None);
}

#[test]
fn a_repeat_closing_where_the_next_opens_is_one_double_repeat_bar() {
    let beat = Beat {
        written: TICKS * 4,
        notes: vec![Note {
            midi: 60,
            tied: false,
        }],
        ..Beat::default()
    };
    let measure = |repeat_start, repeat_end| Measure {
        repeat_start,
        repeat_end,
        voices: vec![vec![beat.clone()]],
        ..Measure::default()
    };
    let imported = Imported {
        parts: vec![Part {
            measures: vec![measure(true, 2), measure(true, 2)],
            ..Part::default()
        }],
        ..Imported::default()
    };
    let abc = to_abc(&imported).expect("the score writes");
    assert_eq!(music(&abc), "|: C8 :: C8 :|");
}

#[test]
fn padding_is_written_in_values_abc_can_draw() {
    // A whole, a quarter and a 64th, and a crumb too short for any value.
    let lengths: Vec<f64> = rests(TICKS * 5 + TICKS / 16 + 7, false)
        .iter()
        .map(|(_, rest)| rest.duration().quarter_length())
        .collect();
    assert_eq!(lengths, [4.0, 1.0, 0.0625]);
}

#[test]
fn a_songsterr_ending_runs_on_to_the_bar_that_closes_the_repeat() {
    let bar = |fields: &str| {
        format!(
            r#"{{{fields} "voices": [{{"beats": [{{"type": 1, "duration": [1, 1], "notes": [{{"string": 1, "fret": 1}}]}}]}}]}}"#
        )
    };
    let json = format!(
        r#"{{"name": "Line", "tuning": [64, 59, 55, 50, 45, 40], "measures": [{}, {}, {}, {}, {}]}}"#,
        bar(r#""repeatStart": true,"#),
        bar(r#""alternateEnding": [1],"#),
        bar(r#""repeat": 2,"#),
        bar(r#""alternateEnding": [2],"#),
        bar(""),
    );
    let imported = read_file("0.json", json.as_bytes()).expect("the file reads");
    let endings: Vec<Vec<u32>> = imported.parts[0]
        .measures
        .iter()
        .map(|measure| measure.ending.clone())
        .collect();
    assert_eq!(endings, [vec![], vec![1], vec![1], vec![2], vec![]]);
    let abc = to_abc(&imported).expect("the score writes");
    assert_eq!(music(&abc), "|: C8 |1 C8 | C8 :|2 C8 || C8 |]");
}

// ------------------------------------------------ the crate's other readers

/// Two bars of G major in MIDI, closing on a chord struck across the
/// barline.
fn midi_file() -> Vec<u8> {
    let mut notes: Vec<music21_rs::MidiNote> = [67, 71, 74, 66, 67]
        .iter()
        .enumerate()
        .map(|(index, &pitch)| {
            music21_rs::MidiNote::new(pitch, index as f64, 1.0, 90).expect("valid note")
        })
        .collect();
    for pitch in [55, 59] {
        notes.push(music21_rs::MidiNote::new(pitch, 5.0, 3.0, 90).expect("valid note"));
    }
    music21_rs::write_midi_bytes(&notes, 100.0).expect("MIDI writes")
}

#[test]
fn midi_is_read_through_the_crate_in_the_key_it_suggests() {
    let abc = abc_of("tune.mid", &midi_file());
    assert!(abc.contains("M:4/4"), "{abc}");
    assert!(abc.contains("K:G"), "{abc}");
    assert!(abc.contains("Q:1/4=100"), "{abc}");
    // F# is in the key signature, so it needs no accidental.
    assert_eq!(music(&abc), "G2 B2 d2 F2 | G2 [G,B,]6 |]");
}

/// A format 1 file of these tracks, 480 ticks to the quarter, each event
/// the sixteenths since the last and its bytes.
fn midi_tracks(tracks: &[&[(u8, &[u8])]]) -> Vec<u8> {
    let mut file = vec![b'M', b'T', b'h', b'd', 0, 0, 0, 6, 0, 1];
    file.extend((tracks.len() as u16).to_be_bytes());
    file.extend(480u16.to_be_bytes());
    for events in tracks {
        let mut body = Vec::new();
        for (sixteenths, event) in events.iter() {
            // Every delta here fits in two bytes of a variable-length number.
            let delta = u16::from(*sixteenths) * 120;
            if delta >= 128 {
                body.push(0x80 | (delta >> 7) as u8);
            }
            body.push((delta & 0x7f) as u8);
            body.extend(*event);
        }
        body.extend([0, 0xff, 0x2f, 0]);
        file.extend(b"MTrk");
        file.extend((body.len() as u32).to_be_bytes());
        file.extend(body);
    }
    file
}

#[test]
fn a_midi_track_keeps_its_name_program_meter_and_tempo() {
    let conductor: &[(u8, &[u8])] = &[
        (0, &[0xff, 0x58, 4, 3, 2, 24, 8]),
        (0, &[0xff, 0x51, 3, 0x09, 0x27, 0xc0]),
    ];
    let flute: &[(u8, &[u8])] = &[
        (0, &[0xff, 3, 5, b'F', b'l', b'u', b't', b'e']),
        (0, &[0xc0, 73]),
        (0, &[0x90, 72, 80]),
        (4, &[0x80, 72, 0]),
        (0, &[0x90, 74, 80]),
        (8, &[0x80, 74, 0]),
    ];
    let abc = abc_of("flute.mid", &midi_tracks(&[conductor, flute]));
    assert!(abc.contains("M:3/4"), "{abc}");
    assert!(abc.contains("Q:1/4=100"), "{abc}");
    assert!(
        abc.contains("V:1 name=\"Flute\"\n%%MIDI program 73"),
        "{abc}"
    );
    assert_eq!(music(&abc), "c2 d4 |]");
}

#[test]
fn a_midi_file_of_drums_alone_is_refused_with_the_reason() {
    let strokes: Vec<music21_rs::MidiNote> = (0..4)
        .map(|beat| {
            music21_rs::MidiNote::with_channel(36, f64::from(beat), 1.0, 100, 9)
                .expect("valid note")
        })
        .collect();
    let bytes = music21_rs::write_midi_bytes(&strokes, 120.0).expect("MIDI writes");
    let imported = read_file("drums.midi", &bytes).expect("the file reads");
    let error = to_abc(&imported).unwrap_err();
    assert!(error.contains("unpitched percussion"), "{error}");
}

const HUMDRUM: &str = "!!!OTL: Two spines\n**kern\t**kern\n*clefF4\t*clefG2\n*k[f#]\t*k[f#]\n*M3/4\t*M3/4\n\
=1\t=1\n2G\t4g\n.\t8a\n.\t8b\n4D\t4cc\n=2\t=2\n2.G\t2.g\n==\t==\n*-\t*-\n";

#[test]
fn humdrum_spines_are_parts_the_last_spine_first() {
    let abc = abc_of("tune.krn", HUMDRUM.as_bytes());
    assert!(abc.contains("T:Two spines"), "{abc}");
    assert!(abc.contains("M:3/4"), "{abc}");
    assert!(abc.contains("K:G"), "{abc}");
    // The lower spine reads in the bass clef, said where it is.
    assert!(abc.contains("V:2 clef=bass name=\"Part\""), "{abc}");
    assert_eq!(music(&abc), "G2 AB c2 | G6 |]");
    assert_eq!(voice_music(&abc, 2), "G,4 D,2 | G,6 |]");
}

const MEI: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<mei xmlns="http://www.music-encoding.org/ns/mei" meiversion="4.0.1">
<meiHead><fileDesc><titleStmt><title>Two staves</title></titleStmt></fileDesc></meiHead>
<music><body><mdiv><score>
<scoreDef meter.count="2" meter.unit="4" key.sig="1f">
  <staffGrp>
    <staffDef n="1" lines="5" clef.shape="G" clef.line="2"/>
    <staffDef n="2" lines="5" clef.shape="F" clef.line="4"/>
  </staffGrp>
</scoreDef>
<section>
  <measure n="1">
    <staff n="1"><layer n="1"><note pname="f" oct="4" dur="4"/><note pname="b" oct="4" dur="4" accid.ges="f"/></layer></staff>
    <staff n="2"><layer n="1"><chord dur="2"><note pname="f" oct="2"/><note pname="c" oct="3"/></chord></layer></staff>
  </measure>
  <measure n="2">
    <staff n="1"><layer n="1"><tuplet num="3" numbase="2"><note pname="a" oct="4" dur="8"/><note pname="g" oct="4" dur="8"/><note pname="f" oct="4" dur="8"/></tuplet><rest dur="4"/></layer></staff>
    <staff n="2"><layer n="1"><note pname="f" oct="2" dur="2"/></layer></staff>
  </measure>
</section>
</score></mdiv></body></music></mei>"#;

#[test]
fn mei_staves_are_parts_and_keep_their_tuplets() {
    let abc = abc_of("tune.mei", MEI.as_bytes());
    assert!(abc.contains("M:2/4"), "{abc}");
    assert!(abc.contains("K:F"), "{abc}");
    // The B is flat by the key, written as `accid.ges`.
    assert_eq!(music(&abc), "F2 B2 | (3AGF z2 |]");
    assert_eq!(voice_music(&abc, 2), "[F,,C,]4 | F,,4 |]");
}

const ROMAN_TEXT: &str = "Composer: Nobody\nTitle: Cadence\nTime Signature: 3/4\n\
m1 G: I b2 IV b3 V7\nm2 I\nm3 NC\n";

#[test]
fn roman_text_writes_each_numeral_under_its_chord() {
    let abc = abc_of("cadence.rntxt", ROMAN_TEXT.as_bytes());
    assert!(abc.contains("T:Cadence"), "{abc}");
    assert!(abc.contains("K:G"), "{abc}");
    assert_eq!(
        music(&abc),
        "\"_G: I\"[GBd]2 \"_IV\"[ceg]2 \"_V7\"[dfac']2 | \"_I\"[GBd]6 | z6 |]"
    );
}

#[test]
fn tiny_notation_is_read_by_name_or_by_its_label() {
    let text = "tinyNotation: 3/4 E4 r f# g trip{b-8 a g} c'2.";
    let abc = abc_of("line.tntxt", text.as_bytes());
    assert!(abc.contains("M:3/4"), "{abc}");
    let music = music(&abc);
    assert!(music.starts_with("E,2 z2 ^F2 | G2 (3"), "{abc}");
    // The last note runs past its barline and is tied into the next bar.
    assert!(music.ends_with("G c2- | c4 |]"), "{abc}");
    assert_eq!(abc_of("line.txt", text.as_bytes()), abc);
}
