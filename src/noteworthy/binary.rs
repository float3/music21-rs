//! NoteWorthy Composer's binary format, `.nwc`: music21's
//! `noteworthy.binaryTranslate`, which reads a file's staves and objects and
//! writes each out as a line of the text format for the text reader.

use super::{error, from_lines};
use crate::error::Result;
use crate::stream::Stream;

/// What a compressed file starts with; zlib's stream follows.
const COMPRESSED: &[u8] = b"[NWZ]\x00";

/// Reads a NoteWorthy Composer `.nwc` file's bytes into a score, as music21's
/// `converter.parse` reads one: each staff and object written out as a line
/// of the text format ([`nwc_lines`]) and those read by
/// [`super::from_noteworthy`]'s reader.
///
/// The file must be uncompressed. A compressed one starts `[NWZ]` and a nul
/// byte, and what follows is a zlib stream: inflate it first, as this crate
/// unpacks nothing.
///
/// music21 calls this reader beta, and what it leaves out is left out here:
/// lyrics, dynamics, flow marks and performance marks are read past and not
/// written.
///
/// # Errors
///
/// A compressed file, a file that ends early, an object of a kind music21
/// does not know, a note or rest in a file older than music21 reads, or
/// anything the text reader refuses.
pub fn from_nwc(bytes: &[u8]) -> Result<Stream> {
    let lines = nwc_lines(bytes)?;
    from_lines(lines.iter().map(String::as_str), false)
}

/// The lines of NoteWorthy text a `.nwc` file's bytes stand for, as
/// music21's `dumpToNWCText` writes them: the title and author, then for
/// each staff its name, its instrument and its objects.
///
/// # Errors
///
/// As [`from_nwc`], but for what only the text reader refuses.
pub fn nwc_lines(bytes: &[u8]) -> Result<Vec<String>> {
    if bytes.starts_with(COMPRESSED) {
        return Err(error(
            "a compressed .nwc file: inflate the zlib stream after its first six bytes first",
        ));
    }
    let mut reader = NwcReader::new(bytes, 200);
    let header = reader.header()?;
    let mut staves = Vec::new();
    for _ in 0..header.number_of_staves {
        staves.push(reader.staff()?);
    }
    Ok(dump_lines(&header, &staves))
}

/// music21's `dumpToNWCText`: the title and author, then each staff's
/// lines.
#[must_use]
pub fn dump_lines(header: &NwcHeader, staves: &[NwcStaff]) -> Vec<String> {
    let mut info = String::new();
    if !header.title.is_empty() {
        info.push_str("|SongInfo|Title:");
        info.push_str(&latin1(&header.title));
    }
    if !header.author.is_empty() {
        info.push_str("|Author:");
        info.push_str(&latin1(&header.author));
    }
    let mut lines = vec![info];
    for staff in staves {
        lines.extend(staff.dump());
    }
    lines
}

/// Bytes read as latin-1, as music21 decodes every string in the file.
fn latin1(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| char::from(*byte)).collect()
}

/// An index into a table as Python takes one: a negative one counts from
/// the end.
fn python_index<T: Copy>(table: &[T], index: i32) -> Option<T> {
    let length = i32::try_from(table.len()).ok()?;
    let index = if index < 0 { index + length } else { index };
    usize::try_from(index)
        .ok()
        .and_then(|index| table.get(index).copied())
}

/// music21's `constants.ClefNames`.
pub const CLEF_NAMES: [&str; 5] = ["Treble", "Bass", "Alto", "Tenor", "Percussion"];

/// music21's `constants.OctaveShiftNames`.
pub const OCTAVE_SHIFT_NAMES: [Option<&str>; 3] = [None, Some("Octave Up"), Some("Octave Down")];

/// music21's `constants.FlatMask`.
pub const FLAT_MASK: [(u8, &str); 8] = [
    (0x00, ""),
    (0x02, "Bb"),
    (0x12, "Bb,Eb"),
    (0x13, "Bb,Eb,Ab"),
    (0x1B, "Bb,Eb,Ab,Db"),
    (0x5B, "Bb,Eb,Ab,Db,Gb"),
    (0x5F, "Bb,Eb,Ab,Db,Gb,Cb"),
    (0x7F, "Bb,Eb,Ab,Db,Gb,Cb,Fb"),
];

/// music21's `constants.SharpMask`.
pub const SHARP_MASK: [(u8, &str); 8] = [
    (0x00, ""),
    (0x20, "F#"),
    (0x24, "F#,C#"),
    (0x64, "F#,C#,G#"),
    (0x6C, "F#,C#,G#,D#"),
    (0x6D, "F#,C#,G#,D#,A#"),
    (0x7D, "F#,C#,G#,D#,A#,E#"),
    (0x7F, "F#,C#,G#,D#,A#,E#,B#"),
];

/// music21's `constants.AlterationTexts`: `x` is a double sharp, `v` a
/// double flat.
pub const ALTERATION_TEXTS: [&str; 6] = ["#", "b", "n", "x", "v", ""];

/// music21's `constants.BarStyles`.
pub const BAR_STYLES: [&str; 8] = [
    "Single",
    "Double",
    "SectionOpen",
    "SectionClose",
    "LocalRepeatOpen",
    "LocalRepeatClose",
    "MasterRepeatOpen",
    "MasterRepeatClose",
];

/// music21's `constants.DurationValues`.
pub const DURATION_VALUES: [&str; 7] = ["Whole", "Half", "4th", "8th", "16th", "32nd", "64th"];

/// music21's `constants.MidiInstruments`, the General MIDI names by program.
pub const MIDI_INSTRUMENTS: [&str; 128] = [
    "Acoustic Grand Piano",
    "Bright Acoustic Piano",
    "Electric Grand Piano",
    "Honky-tonk Piano",
    "Electric Piano 1",
    "Electric Piano 2",
    "Harpsichord",
    "Clavi",
    "Celesta",
    "Glockenspiel",
    "Music Box",
    "Vibraphone",
    "Marimba",
    "Xylophone",
    "Tubular Bells",
    "Dulcimer",
    "Drawbar Organ",
    "Percussive Organ",
    "Rock Organ",
    "Church Organ",
    "Reed Organ",
    "Accordion",
    "Harmonica",
    "Tango Accordion",
    "Acoustic Guitar (nylon)",
    "Acoustic Guitar (steel)",
    "Electric Guitar (jazz)",
    "Electric Guitar (clean)",
    "Electric Guitar (muted)",
    "Overdriven Guitar",
    "Distortion Guitar",
    "Guitar harmonics",
    "Acoustic Bass",
    "Electric Bass (finger)",
    "Electric Bass (pick)",
    "Fretless Bass",
    "Slap Bass 1",
    "Slap Bass 2",
    "Synth Bass 1",
    "Synth Bass 2",
    "Violin",
    "Viola",
    "Cello",
    "Contrabass",
    "Tremolo Strings",
    "Pizzicato Strings",
    "Orchestral Harp",
    "Timpani",
    "String Ensemble 1",
    "String Ensemble 2",
    "SynthStrings 1",
    "SynthStrings 2",
    "Choir Aahs",
    "Voice Oohs",
    "Synth Voice",
    "Orchestra Hit",
    "Trumpet",
    "Trombone",
    "Tuba",
    "Muted Trumpet",
    "French Horn",
    "Brass Section",
    "SynthBrass 1",
    "SynthBrass 2",
    "Soprano Sax",
    "Alto Sax",
    "Tenor Sax",
    "Baritone Sax",
    "Oboe",
    "English Horn",
    "Bassoon",
    "Clarinet",
    "Piccolo",
    "Flute",
    "Recorder",
    "Pan Flute",
    "Blown Bottle",
    "Shakuhachi",
    "Whistle",
    "Ocarina",
    "Lead 1 (square)",
    "Lead 2 (sawtooth)",
    "Lead 3 (calliope)",
    "Lead 4 (chiff)",
    "Lead 5 (charang)",
    "Lead 6 (voice)",
    "Lead 7 (fifths)",
    "Lead 8 (bass + lead)",
    "Pad 1 (new age)",
    "Pad 2 (warm)",
    "Pad 3 (polysynth)",
    "Pad 4 (choir)",
    "Pad 5 (bowed)",
    "Pad 6 (metallic)",
    "Pad 7 (halo)",
    "Pad 8 (sweep)",
    "FX 1 (rain)",
    "FX 2 (soundtrack)",
    "FX 3 (crystal)",
    "FX 4 (atmosphere)",
    "FX 5 (brightness)",
    "FX 6 (goblins)",
    "FX 7 (echoes)",
    "FX 8 (sci-fi)",
    "Sitar",
    "Banjo",
    "Shamisen",
    "Koto",
    "Kalimba",
    "Bag pipe",
    "Fiddle",
    "Shanai",
    "Tinkle Bell",
    "Agogo",
    "Steel Drums",
    "Woodblock",
    "Taiko Drum",
    "Melodic Tom",
    "Synth Drum",
    "Reverse Cymbal",
    "Guitar Fret Noise",
    "Breath Noise",
    "Seashore",
    "Bird Tweet",
    "Telephone Ring",
    "Helicopter",
    "Applause",
    "Gunshot",
];

/// A font the header names: music21's `FontDict`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NwcFont {
    /// Its name, `Times New Roman` where the file gives none.
    pub name: Vec<u8>,
    /// Regular, bold, italic or both, as the file numbers them.
    pub style: u8,
    /// Its size in points, 12 where the file gives none.
    pub size: u8,
    /// Its character set.
    pub charset: u8,
}

/// What a file's header says: music21's `NWCConverter.parseHeader`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[allow(missing_docs)]
pub struct NwcHeader {
    pub version: u32,
    pub user: Vec<u8>,
    pub title: Vec<u8>,
    pub author: Vec<u8>,
    pub lyricist: Vec<u8>,
    pub copyright1: Vec<u8>,
    pub copyright2: Vec<u8>,
    pub comment: Vec<u8>,
    pub extend_last_system: u8,
    pub increase_note_spacing: u8,
    pub measure_numbers: u8,
    pub measure_start: i32,
    pub margins: Vec<u8>,
    pub group_visibility: Vec<u8>,
    pub allow_layering: u8,
    pub notation_typeface: Vec<u8>,
    pub staff_height: i32,
    pub fonts: Vec<NwcFont>,
    pub title_page_info: u8,
    pub staff_labels: u8,
    pub page_number_start: i32,
    pub number_of_staves: u8,
}

/// A staff and its objects: music21's `NWCStaff`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[allow(missing_docs)]
pub struct NwcStaff {
    pub name: Vec<u8>,
    pub label: Vec<u8>,
    pub instrument_name: Vec<u8>,
    pub group: Vec<u8>,
    pub lines: u8,
    pub layer_with_next_staff: Option<i32>,
    /// Semitones up, where the file says; music21 writes `None` otherwise.
    pub transposition: Option<i32>,
    pub part_volume: Option<i32>,
    pub stereo_pan: Option<i32>,
    pub color: u8,
    pub align_syllable: Option<i32>,
    pub number_of_lyrics: i32,
    pub lyric_alignment: i32,
    pub staff_offset: i32,
    /// Each verse's syllables.
    pub lyrics: Vec<Vec<Vec<u8>>>,
    pub number_of_objects: i32,
    pub objects: Vec<NwcObject>,
}

impl NwcStaff {
    /// music21's `NWCStaff.dump`: the staff and its instrument as text
    /// lines, then each object that writes one.
    #[must_use]
    pub fn dump(&self) -> Vec<String> {
        let instrument = if self.instrument_name.is_empty() {
            "Acoustic Grand Piano".to_string()
        } else {
            latin1(&self.instrument_name)
        };
        let label = if self.label.is_empty() {
            instrument.clone()
        } else {
            latin1(&self.label)
        };
        let mut lines = vec![format!("|AddStaff|Name:{label}")];
        let mut staff_instrument = format!("|StaffInstrument|Name:{instrument}");
        if let Some(patch) = MIDI_INSTRUMENTS.iter().position(|name| *name == instrument) {
            staff_instrument.push_str(&format!("|Patch:{patch}"));
        }
        staff_instrument.push_str(&format!(
            "|Trans:{}",
            self.transposition
                .map_or("None".to_string(), |semitones| semitones.to_string())
        ));
        lines.push(staff_instrument);
        lines.extend(
            self.objects
                .iter()
                .map(NwcObject::line)
                .filter(|line| !line.is_empty()),
        );
        lines
    }
}

/// One object of a staff: music21's `NWCObject`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NwcObject {
    /// Whether and where it is shown; nought before version 1.70.
    pub visible: u8,
    /// What it is.
    pub body: Object,
}

impl NwcObject {
    /// music21's `type`: the name of the method that read it.
    #[must_use]
    pub fn type_name(&self) -> &'static str {
        match &self.body {
            Object::Silent(name) => name,
            Object::Clef { .. } => "Clef",
            Object::Key(_) => "KeySig",
            Object::Bar(_) => "Barline",
            Object::Ending(_) => "Ending",
            Object::TimeSig(..) => "TimeSig",
            Object::Tempo(_) => "Tempo",
            Object::Note(_) => "Note",
            Object::Rest(_) => "Rest",
            Object::Chord { .. } => "NoteChordMember",
            Object::RestChord { .. } => "RestChordMember",
            Object::Text(..) => "Text",
        }
    }

    /// music21's `dumpMethod`: the text line, or nothing.
    #[must_use]
    pub fn line(&self) -> String {
        self.body.line()
    }
}

/// What an object is, as much of it as is written out.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Object {
    /// Read past and written as nothing, with music21's `type` for it.
    Silent(&'static str),
    /// A clef by name, with its octave shift.
    Clef {
        /// Its name in `CLEF_NAMES`.
        name: Option<&'static str>,
        /// Its octave shift in `OCTAVE_SHIFT_NAMES`.
        shift: Option<&'static str>,
    },
    /// A key signature's `Signature` text.
    Key(&'static str),
    /// A barline by its style's number in `BAR_STYLES`.
    Bar(u8),
    /// An ending by its number.
    Ending(u8),
    /// A meter's numerator and denominator.
    TimeSig(i32, i64),
    /// A tempo in beats a minute.
    Tempo(i32),
    /// A note.
    Note(Member),
    /// A rest's `Dur` text.
    Rest(String),
    /// A chord's notes, and the stem length its bytes give.
    Chord {
        /// Its notes.
        members: Vec<Member>,
        /// Its stem length.
        stem_length: u8,
    },
    /// A rest's `Dur` text and the notes sounding with it.
    RestChord {
        /// The notes.
        members: Vec<Member>,
        /// The rest's length.
        rest: String,
    },
    /// Words and their staff position.
    Text(Vec<u8>, i32),
}

/// A note as a chord or a staff holds it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Member {
    /// Its `Dur` text: music21's `durationStr`.
    pub duration: String,
    /// Its staff position: music21's `pos`.
    pub position: i32,
    /// The alteration in force there: music21's `alterationStr`.
    pub alteration: String,
    /// `^` where it is tied on: music21's `tieInfo`.
    pub tie: &'static str,
    /// Its stem length.
    pub stem_length: u8,
}

impl Member {
    /// Its `Pos` text: alteration, position and tie.
    #[must_use]
    pub fn pos_text(&self) -> String {
        format!("{}{}{}", self.alteration, self.position, self.tie)
    }
}

impl Object {
    /// music21's `dumpMethod`: the text line, or nothing.
    fn line(&self) -> String {
        match self {
            Self::Silent(_) => String::new(),
            Self::Clef { name, shift } => {
                let mut line = "|Clef|".to_string();
                if let Some(name) = name {
                    line.push_str(&format!("Type:{name}|"));
                }
                if let Some(shift) = shift {
                    line.push_str(&format!("OctaveShift:{shift}|"));
                }
                line
            }
            Self::Key(signature) => format!("|Key|Signature:{signature}"),
            Self::Bar(style) => {
                let mut line = "|Bar|".to_string();
                if *style > 0
                    && let Some(name) = BAR_STYLES.get(usize::from(*style))
                {
                    line.push_str(&format!("|Style:{name}"));
                }
                line
            }
            Self::Ending(style) => format!("|Ending|Endings:{style}"),
            Self::TimeSig(numerator, denominator) => {
                format!("|TimeSig|Signature:{numerator}/{denominator}")
            }
            Self::Tempo(value) => format!("|Tempo|Tempo:{value}"),
            Self::Note(member) => {
                format!("|Note|Dur:{}|Pos:{}|", member.duration, member.pos_text())
            }
            Self::Rest(duration) => format!("|Rest|Dur:{duration}|"),
            Self::Chord { members, .. } => {
                let mut line = "|Chord".to_string();
                for (duration, positions) in by_duration(members) {
                    line.push_str(&format!("|Dur:{duration}|Pos:{positions}"));
                }
                line
            }
            Self::RestChord { members, rest } => match members.first() {
                None => format!("|Rest|Dur:{rest}"),
                Some(first) => {
                    let positions: Vec<String> = members.iter().map(Member::pos_text).collect();
                    format!(
                        "|RestChord|Dur:{rest}|Dur2:{}|Pos2:{}",
                        first.duration,
                        positions.join(",")
                    )
                }
            },
            Self::Text(text, position) => {
                format!("|Text|Text:{}|Pos:{position}", latin1(text))
            }
        }
    }
}

/// The stem length music21 gives a note or chord whose bytes name none.
const DEFAULT_STEM: u8 = 7;

/// The bit of a note's or chord's last byte that says a stem length follows.
const STEM_FOLLOWS: u8 = 0x40;

/// The music21 `type` of each object kind, by the number the file gives.
const OBJECT_TYPES: [&str; 19] = [
    "Clef",
    "KeySig",
    "Barline",
    "Ending",
    "Instrument",
    "TimeSig",
    "Tempo",
    "Dynamic",
    "Note",
    "Rest",
    "NoteChordMember",
    "Pedal",
    "FlowDir",
    "MPC",
    "TempoVariation",
    "DynamicVariation",
    "Performance",
    "Text",
    "RestChordMember",
];

/// music21's `NWCConverter` as a reader: the file, where it has got to in
/// it, its version, and the alteration last written on each staff
/// position, by the position modulo seven, until a barline.
#[derive(Clone, Debug)]
pub struct NwcReader<'a> {
    data: &'a [u8],
    /// Where the next read starts: music21's `parsePosition`.
    pub at: usize,
    /// The file's version, as music21 numbers it: 175 for 1.75.
    pub version: u32,
    /// music21's `currentAlterations`.
    pub alterations: Vec<(i32, String)>,
}

impl<'a> NwcReader<'a> {
    /// A reader over a file's (inflated) bytes, at its start, taken to be
    /// of `version` until its header says.
    #[must_use]
    pub fn new(data: &'a [u8], version: u32) -> Self {
        Self {
            data,
            at: 0,
            version,
            alterations: Vec::new(),
        }
    }

    fn ended() -> crate::error::Error {
        error("the file ends before its last object")
    }

    /// A little-endian signed short: `readLEShort`.
    fn short(&mut self) -> Result<i32> {
        let bytes = self
            .data
            .get(self.at..self.at + 2)
            .ok_or_else(Self::ended)?;
        self.at += 2;
        Ok(i32::from(i16::from_le_bytes([bytes[0], bytes[1]])))
    }

    /// `byteToInt`.
    fn byte(&mut self) -> Result<u8> {
        let byte = *self.data.get(self.at).ok_or_else(Self::ended)?;
        self.at += 1;
        Ok(byte)
    }

    /// `byteToSignedInt`.
    fn signed(&mut self) -> Result<i32> {
        Ok(i32::from(self.byte()? as i8))
    }

    /// `readBytes`: as many of the next `count` bytes as there are.
    fn take(&mut self, count: usize) -> Vec<u8> {
        let start = self.at.min(self.data.len());
        let end = (self.at + count).min(self.data.len());
        self.at += count;
        self.data[start..end].to_vec()
    }

    fn skip(&mut self, count: usize) {
        self.at += count;
    }

    /// `readToNUL`: the bytes up to the next nul, which is passed. With no
    /// nul left, the rest -- and music21 then starts again from the file's
    /// first byte.
    fn until_nul(&mut self) -> Vec<u8> {
        let start = self.at.min(self.data.len());
        match self.data[start..].iter().position(|byte| *byte == 0) {
            Some(length) => {
                self.at = start + length + 1;
                self.data[start..start + length].to_vec()
            }
            None => {
                self.at = 0;
                self.data[start..].to_vec()
            }
        }
    }

    /// `advanceToNotNUL`.
    fn past_nuls(&mut self) {
        while self.data.get(self.at) == Some(&0) {
            self.at += 1;
        }
    }

    /// music21's `parseHeader`, with `fileVersion` read where music21
    /// reads it. The version read is kept.
    ///
    /// # Errors
    ///
    /// A file that ends inside its header.
    pub fn header(&mut self) -> Result<NwcHeader> {
        self.at = 45;
        self.version = match self.short()? {
            0x0114 => 120,
            0x011E => 130,
            0x0132 => 150,
            0x0137 => 155,
            0x0146 => 170,
            0x014B => 175,
            0x0200 => 200,
            0x0201 => 201,
            // Most likely a newer version.
            _ => 201,
        };
        let mut header = NwcHeader {
            version: self.version,
            ..NwcHeader::default()
        };
        self.skip(4);
        header.user = self.until_nul();
        let _unknown = self.until_nul();
        self.skip(10);
        header.title = self.until_nul();
        header.author = self.until_nul();
        if self.version >= 200 {
            header.lyricist = self.until_nul();
        }
        header.copyright1 = self.until_nul();
        header.copyright2 = self.until_nul();
        header.comment = self.until_nul();
        header.extend_last_system = self.byte()?;
        header.increase_note_spacing = self.byte()?;
        self.take(5);
        header.measure_numbers = self.byte()?;
        self.take(1);
        header.measure_start = self.short()?;
        header.margins = if self.version >= 130 {
            self.until_nul()
        } else {
            b"0.0 0.0 0.0 0.0".to_vec()
        };
        let _unused = self.byte()?;
        self.take(2);
        header.allow_layering = 1;
        if self.version >= 130 {
            header.group_visibility = self.take(32);
            header.allow_layering = self.byte()?;
        }
        header.notation_typeface = if self.version >= 200 {
            self.until_nul()
        } else {
            b"Maestro".to_vec()
        };
        header.staff_height = self.short()?;
        let fonts = if self.version > 170 {
            12
        } else if self.version > 130 {
            10
        } else {
            0
        };
        self.past_nuls();
        self.skip(2);
        for _ in 0..fonts {
            let mut font = NwcFont {
                name: self.until_nul(),
                style: self.byte()?,
                size: self.byte()?,
                charset: 0,
            };
            let _unused = self.byte()?;
            font.charset = self.byte()?;
            if font.name.is_empty() {
                font.name = b"Times New Roman".to_vec();
            }
            if font.size == 0 {
                font.size = 12;
            }
            header.fonts.push(font);
        }
        header.title_page_info = self.byte()?;
        header.staff_labels = self.byte()?;
        header.page_number_start = self.short()?;
        if self.version >= 200 {
            self.skip(1);
        }
        header.number_of_staves = self.byte()?;
        self.skip(1);
        Ok(header)
    }

    /// music21's `NWCStaff.parse`: a staff's header, its verses and its
    /// objects.
    ///
    /// # Errors
    ///
    /// A file that ends inside the staff, or an object music21 cannot read.
    pub fn staff(&mut self) -> Result<NwcStaff> {
        let mut staff = self.staff_header()?;
        staff.lyrics = self.staff_lyrics(staff.number_of_lyrics)?;
        (staff.number_of_objects, staff.objects) = self.staff_objects()?;
        Ok(staff)
    }

    /// `NWCStaff.parseObjects`: how many objects the staff says it has,
    /// and the objects.
    ///
    /// # Errors
    ///
    /// As [`Self::staff`].
    pub fn staff_objects(&mut self) -> Result<(i32, Vec<NwcObject>)> {
        let mut count = self.short()?;
        if self.version > 150 {
            count -= 2;
        }
        let mut objects = Vec::new();
        for _ in 0..count.max(0) {
            objects.push(self.object()?);
        }
        Ok((count, objects))
    }

    /// `NWCStaff.parseHeader`: a staff's names, settings and how many
    /// verses it has.
    ///
    /// # Errors
    ///
    /// As [`Self::staff`].
    pub fn staff_header(&mut self) -> Result<NwcStaff> {
        let mut staff = NwcStaff {
            name: self.until_nul(),
            ..NwcStaff::default()
        };
        if self.version >= 200 {
            staff.label = self.until_nul();
            staff.instrument_name = self.until_nul();
        }
        staff.group = self.until_nul();
        if self.version >= 200 {
            self.skip(27);
            staff.lines = self.byte()?;
            staff.layer_with_next_staff = Some(self.short()?);
            staff.transposition = Some(self.short()?);
            staff.part_volume = Some(self.short()?);
            staff.stereo_pan = Some(self.short()?);
            staff.color = self.byte()?;
            staff.align_syllable = Some(self.short()?);
            staff.number_of_lyrics = self.short()?;
        } else if self.version == 175 {
            self.skip(11);
            let patch = usize::from(self.byte()?);
            let index = if patch > 0 && patch < MIDI_INSTRUMENTS.len() {
                patch - 1
            } else {
                0
            };
            staff.instrument_name = MIDI_INSTRUMENTS[index].bytes().collect();
            self.skip(10);
            staff.transposition = Some(self.signed()?);
            self.skip(6);
            staff.align_syllable = Some(self.short()?);
            staff.number_of_lyrics = self.short()?;
        }
        if staff.number_of_lyrics > 0 {
            staff.lyric_alignment = self.short()?;
            staff.staff_offset = self.short()?;
        }
        Ok(staff)
    }

    /// `NWCStaff.parseLyrics`: each verse's syllables.
    ///
    /// # Errors
    ///
    /// As [`Self::staff`].
    pub fn staff_lyrics(&mut self, verses: i32) -> Result<Vec<Vec<Vec<u8>>>> {
        let mut lyrics = Vec::new();
        for _ in 0..verses {
            let size = self.short().unwrap_or(0);
            if size <= 0 {
                continue;
            }
            let _length = self.short()?;
            let start = self.at;
            let _junk = self.short()?;
            let mut syllables = Vec::new();
            for _ in 0..1000 {
                let syllable = self.until_nul();
                if syllable.is_empty() {
                    break;
                }
                syllables.push(syllable);
            }
            self.at = start.saturating_add_signed(size as isize);
            lyrics.push(syllables);
        }
        if verses > 0 {
            let _junk = self.short()?;
        }
        let _junk = self.short()?;
        Ok(lyrics)
    }

    /// music21's `NWCObject.parse`: an object of the type its first short
    /// names.
    ///
    /// # Errors
    ///
    /// An object of a kind music21 does not know, a file that ends inside
    /// it, or a note or rest in a file older than music21 reads.
    pub fn object(&mut self) -> Result<NwcObject> {
        let kind = self.short()?;
        let Some(name) = usize::try_from(kind)
            .ok()
            .and_then(|kind| OBJECT_TYPES.get(kind))
        else {
            return Err(error(format!(
                "Cannot translate objectType: {kind}; max is {}",
                OBJECT_TYPES.len()
            )));
        };
        let visible = if self.version >= 170 { self.byte()? } else { 0 };
        let body = match kind {
            0 => self.clef()?,
            1 => self.key()?,
            2 => {
                let style = self.byte()?;
                let _repeats = self.byte()?;
                self.alterations.clear();
                Object::Bar(style)
            }
            3 => {
                let style = self.byte()?;
                self.skip(1);
                Object::Ending(style)
            }
            4 => {
                self.skip(8);
                Object::Silent(name)
            }
            5 => {
                let numerator = self.short()?;
                let bits = self.short()?;
                let _style = self.short()?;
                let denominator = u32::try_from(bits)
                    .ok()
                    .and_then(|bits| 1_i64.checked_shl(bits))
                    .ok_or_else(|| error(format!("a meter of 2 to the power {bits}")))?;
                Object::TimeSig(numerator, denominator)
            }
            6 => {
                let _position = self.byte()?;
                let _placement = self.byte()?;
                let value = self.short()?;
                let _base = self.byte()?;
                if self.version < 170 {
                    let _junk = self.short()?;
                }
                let _text = self.until_nul();
                Object::Tempo(value)
            }
            7 => {
                if self.version >= 170 {
                    self.take(3);
                    let _velocity = self.short()?;
                    let _volume = self.short()?;
                }
                Object::Silent(name)
            }
            8 => Object::Note(self.note()?),
            9 => {
                if self.version <= 150 {
                    return Err(error("a rest in a file older than version 1.70"));
                }
                let duration = self.byte()?;
                let data = self.take(5);
                let _offset = self.short()?;
                Object::Rest(duration_text(duration, &data, None)?)
            }
            10 => {
                let (members, _, stem_length) = self.chord()?;
                Object::Chord {
                    members,
                    stem_length,
                }
            }
            11 => {
                if self.version >= 170 {
                    self.take(3);
                }
                Object::Silent(name)
            }
            12 => {
                if self.version >= 170 {
                    self.take(2);
                }
                let _style = self.short()?;
                Object::Silent(name)
            }
            13 => {
                self.take(2);
                if self.version == 175 || self.version <= 155 {
                    self.take(32);
                } else {
                    self.take(31);
                }
                Object::Silent(name)
            }
            14 => {
                self.take(4);
                Object::Silent(name)
            }
            15 | 16 => {
                let _position = self.byte()?;
                if self.version >= 170 {
                    let _placement = self.byte()?;
                }
                let _style = self.byte()?;
                Object::Silent(name)
            }
            17 => {
                let position = self.signed()?;
                let _data = self.byte()?;
                let _font = self.byte()?;
                Object::Text(self.until_nul(), position)
            }
            _ => self.rest_chord()?,
        };
        Ok(NwcObject { visible, body })
    }

    /// `clef`: its type and octave shift, named where music21 has a name.
    fn clef(&mut self) -> Result<Object> {
        let clef = self.short()?;
        let shift = self.short()?;
        let clef_count = i32::try_from(CLEF_NAMES.len()).unwrap_or(0);
        let shift_count = i32::try_from(OCTAVE_SHIFT_NAMES.len()).unwrap_or(0);
        let name = if clef < clef_count {
            Some(
                python_index(&CLEF_NAMES, clef)
                    .ok_or_else(|| error(format!("no clef numbered {clef}")))?,
            )
        } else {
            None
        };
        let shift = if shift < shift_count {
            python_index(&OCTAVE_SHIFT_NAMES, shift)
                .ok_or_else(|| error(format!("no octave shift numbered {shift}")))?
        } else {
            None
        };
        Ok(Object::Clef { name, shift })
    }

    /// `keySig`: the flats written, else the sharps.
    fn key(&mut self) -> Result<Object> {
        let flats = self.byte()?;
        self.skip(1);
        let sharps = self.byte()?;
        self.skip(7);
        let mask = |table: &[(u8, &'static str)], key: u8| {
            table
                .iter()
                .find(|(known, _)| *known == key)
                .map(|(_, text)| *text)
        };
        let signature = if flats > 0
            && let Some(text) = mask(&FLAT_MASK, flats)
        {
            text
        } else if sharps > 0
            && let Some(text) = mask(&SHARP_MASK, sharps)
        {
            text
        } else {
            ""
        };
        Ok(Object::Key(signature))
    }

    /// `restChordMember`: a rest's bytes and the notes sounding with it.
    /// Only a file of version 1.75 writes one as it writes a chord; the
    /// others give the rest's length, its five bytes and its offset, then
    /// how many notes follow.
    fn rest_chord(&mut self) -> Result<Object> {
        let (members, data) = if 170 < self.version && self.version < 200 {
            let (members, data, _) = self.chord()?;
            (members, data)
        } else {
            let data = self.take(8);
            let notes = usize::try_from(self.short()?).unwrap_or(0);
            (self.members(notes)?, data)
        };
        let duration = *data.first().ok_or_else(Self::ended)?;
        let rest_data = data.get(1..6).ok_or_else(Self::ended)?;
        Ok(Object::RestChord {
            members,
            rest: duration_text(duration, rest_data, None)?,
        })
    }

    /// `note`: a note's length, its position on the staff with the
    /// alteration in force there, whether it is tied on, and its stem.
    fn note(&mut self) -> Result<Member> {
        if self.version < 170 {
            return Err(error("a note in a file older than version 1.70"));
        }
        let duration = self.byte()?;
        let data = self.take(3);
        let attributes = self.take(2);
        let position = -self.signed()?;
        let second = self.byte()?;
        if self.version <= 170 {
            self.take(2);
        }
        let stem_length = if self.version >= 200 && second & STEM_FOLLOWS != 0 {
            self.byte()?
        } else {
            DEFAULT_STEM
        };
        let duration = duration_text(duration, &data, Some(&attributes))?;
        let mut alteration = ALTERATION_TEXTS
            .get(usize::from(second & 0x07))
            .copied()
            .unwrap_or("")
            .to_string();
        let place = position.rem_euclid(7);
        if alteration.is_empty() {
            alteration = self
                .alterations
                .iter()
                .find(|(known, _)| *known == place)
                .map(|(_, text)| text.clone())
                .unwrap_or_default();
        }
        self.alterations.retain(|(known, _)| *known != place);
        self.alterations.push((place, alteration.clone()));
        let tie = if attributes.first().is_some_and(|first| first & 0x10 != 0) {
            "^"
        } else {
            ""
        };
        Ok(Member {
            duration,
            position,
            alteration,
            tie,
            stem_length,
        })
    }

    /// `noteChordMember`: the chord's notes, its own bytes and its stem
    /// length.
    fn chord(&mut self) -> Result<(Vec<Member>, Vec<u8>, u8)> {
        let mut notes = 0;
        let data = if self.version <= 170 {
            let data = self.take(12);
            notes = usize::try_from(self.short()?).unwrap_or(0);
            data
        } else if self.version == 175 {
            let data = self.take(10);
            notes = usize::from(*data.get(8).ok_or_else(Self::ended)?);
            data
        } else {
            self.take(8)
        };
        let mut stem_length = DEFAULT_STEM;
        if self.version >= 200 {
            if data.get(7).ok_or_else(Self::ended)? & STEM_FOLLOWS != 0 {
                stem_length = self.byte()?;
            }
            notes = usize::try_from(self.short()?).unwrap_or(0);
        }
        Ok((self.members(notes)?, data, stem_length))
    }

    /// `_readChordNotes`: the notes of a chord.
    fn members(&mut self, notes: usize) -> Result<Vec<Member>> {
        let mut members = Vec::new();
        for _ in 0..notes {
            match self.object()?.body {
                Object::Note(member) => members.push(member),
                // A rest is written at position nought, as music21's
                // object starts out.
                Object::Rest(duration) => members.push(Member {
                    duration,
                    position: 0,
                    alteration: String::new(),
                    tie: "",
                    stem_length: 0,
                }),
                _ => return Err(error("a chord holding something other than a note")),
            }
        }
        Ok(members)
    }
}

/// A chord's notes grouped by length, in the order each length first
/// appears, with their positions joined, as music21's chord `dump` writes
/// them.
fn by_duration(members: &[Member]) -> Vec<(&str, String)> {
    let mut groups: Vec<(&str, Vec<String>)> = Vec::new();
    for member in members {
        match groups
            .iter_mut()
            .find(|(duration, _)| *duration == member.duration)
        {
            Some((_, positions)) => positions.push(member.pos_text()),
            None => groups.push((&member.duration, vec![member.pos_text()])),
        }
    }
    groups
        .into_iter()
        .map(|(duration, positions)| (duration, positions.join(",")))
        .collect()
}

/// `setDurationForObject`: a note's or rest's `Dur` text, from its length
/// and the bytes saying its dots, whether it is a grace note (a note's
/// `attributes`) and whether it is a triplet.
fn duration_text(duration: u8, data: &[u8], attributes: Option<&[u8]>) -> Result<String> {
    let mut text = DURATION_VALUES
        .get(usize::from(duration))
        .ok_or_else(|| error(format!("no note length numbered {duration}")))?
        .to_string();
    let (dots, grace) = match attributes {
        Some(attributes) => (
            *attributes.first().ok_or_else(NwcReader::ended)?,
            attributes.get(1).ok_or_else(NwcReader::ended)? & 0x20,
        ),
        None => (*data.get(3).ok_or_else(NwcReader::ended)?, 0),
    };
    let triplet = data.get(1).ok_or_else(NwcReader::ended)? & 0x0c;
    if dots & 0x01 != 0 {
        text.push_str(",DblDotted");
    } else if dots & 0x04 != 0 {
        text.push_str(",Dotted");
    }
    if grace != 0 {
        text.push_str(",Grace");
    }
    if triplet != 0 {
        text.push_str(",Triplet");
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_compressed_file_is_left_to_the_caller_to_inflate() {
        let error = nwc_lines(b"[NWZ]\x00\x78\x9c").unwrap_err();
        assert!(error.to_string().contains("inflate"), "{error}");
    }

    #[test]
    fn a_table_is_indexed_as_python_indexes_one() {
        assert_eq!(python_index(&CLEF_NAMES, 1), Some("Bass"));
        assert_eq!(python_index(&CLEF_NAMES, -1), Some("Percussion"));
        assert_eq!(python_index(&CLEF_NAMES, 5), None);
        assert_eq!(python_index(&CLEF_NAMES, -6), None);
    }

    fn read(data: &[u8]) -> String {
        NwcReader::new(data, 201).object().unwrap().line()
    }

    #[test]
    fn a_tenor_and_a_percussion_clef_are_named() {
        // An object of kind 0, shown, a clef type and no octave shift.
        assert_eq!(read(&[0, 0, 0, 3, 0, 0, 0]), "|Clef|Type:Tenor|");
        assert_eq!(read(&[0, 0, 0, 4, 0, 0, 0]), "|Clef|Type:Percussion|");
    }

    #[test]
    fn a_version_two_chord_reads_its_notes() {
        // A chord of quarters and its count, then two notes on positions
        // nought and minus two.
        let mut chord = vec![10, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 2, 0];
        for position in [0u8, (-2i8) as u8] {
            chord.extend([8, 0, 0, 2, 0, 0, 0, 0, 0, position, 5]);
        }
        assert_eq!(read(&chord), "|Chord|Dur:4th|Pos:0,2");
    }

    #[test]
    fn a_chord_groups_its_notes_by_length() {
        let member = |duration: &str, position: i32, alteration: &str, tie| Member {
            duration: duration.to_string(),
            position,
            alteration: alteration.to_string(),
            tie,
            stem_length: DEFAULT_STEM,
        };
        let chord = Object::Chord {
            members: vec![
                member("4th", 1, "", ""),
                member("Half", 3, "", ""),
                member("4th", 5, "#", "^"),
            ],
            stem_length: DEFAULT_STEM,
        };
        assert_eq!(chord.line(), "|Chord|Dur:4th|Pos:1,#5^|Dur:Half|Pos:3");
    }

    #[test]
    fn a_version_two_rest_chord_gives_its_rest_and_then_its_notes() {
        // A half rest, its five bytes, an offset of minus two and a count,
        // then a quarter on position nought.
        let mut chord = vec![18, 0, 0, 1, 0, 0, 0, 0, 0, 0xfe, 0xff, 1, 0];
        chord.extend([8, 0, 0, 2, 0, 0, 0, 0, 0, 0, 5]);
        assert_eq!(read(&chord), "|RestChord|Dur:Half|Dur2:4th|Pos2:0");
        // With no notes it is a rest, dotted where its fourth byte says so.
        assert_eq!(
            read(&[18, 0, 0, 1, 0, 0, 0, 0x04, 0, 0, 0, 0, 0]),
            "|Rest|Dur:Half,Dotted"
        );
    }

    #[test]
    fn a_version_two_chord_may_give_its_stem_length() {
        // The last of the chord's eight bytes flags a stem length, which
        // comes before the count; a treble clef follows the two notes.
        let mut data = vec![10, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0x40, 5, 2, 0];
        for position in [0u8, (-2i8) as u8] {
            data.extend([8, 0, 0, 2, 0, 0, 0, 0, 0, position, 5]);
        }
        data.extend([0, 0, 0, 0, 0, 0, 0]);
        let mut reader = NwcReader::new(&data, 201);
        let chord = reader.object().unwrap();
        assert_eq!(chord.type_name(), "NoteChordMember");
        assert!(matches!(chord.body, Object::Chord { stem_length: 5, .. }));
        assert_eq!(reader.object().unwrap().type_name(), "Clef");
        assert_eq!(reader.at, data.len());
    }

    #[test]
    fn a_length_says_its_dots_grace_and_triplet() {
        assert_eq!(
            duration_text(2, &[0, 0, 0, 0x04], None).unwrap(),
            "4th,Dotted"
        );
        assert_eq!(
            duration_text(3, &[0, 0x04, 0], Some(&[0x01, 0x20])).unwrap(),
            "8th,DblDotted,Grace,Triplet"
        );
        assert!(duration_text(7, &[0, 0, 0, 0], None).is_err());
    }
}
