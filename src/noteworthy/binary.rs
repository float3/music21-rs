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
/// written; a chord in a file of version 2 or later holds no notes, which
/// the text reader refuses; and music21's table of clef names runs `Tenor`
/// and `Percussion` together for want of a comma, so a tenor or percussion
/// clef is refused too.
///
/// # Errors
///
/// A compressed file, a file that ends early, an object of a kind music21
/// does not know, a note or rest in a file older than music21 reads, or
/// anything the text reader refuses.
pub fn from_nwc(bytes: &[u8]) -> Result<Stream> {
    let lines = nwc_lines(bytes)?;
    from_lines(lines.iter().map(String::as_str))
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
    let mut reader = Bytes {
        data: bytes,
        at: 0,
        version: 200,
        alterations: Vec::new(),
    };
    let header = reader.header()?;
    let mut lines = Vec::new();
    let mut info = String::new();
    if !header.title.is_empty() {
        info.push_str("|SongInfo|Title:");
        info.push_str(&latin1(&header.title));
    }
    if !header.author.is_empty() {
        info.push_str("|Author:");
        info.push_str(&latin1(&header.author));
    }
    lines.push(info);
    for _ in 0..header.staves {
        let staff = reader.staff()?;
        lines.extend(staff);
    }
    Ok(lines)
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

/// music21's `constants.ClefNames`, as music21 writes it: a missing comma
/// joins its last two names.
const CLEF_NAMES: [&str; 4] = ["Treble", "Bass", "Alto", "TenorPercussion"];

/// music21's `constants.OctaveShiftNames`.
const OCTAVE_SHIFT_NAMES: [Option<&str>; 3] = [None, Some("Octave Up"), Some("Octave Down")];

/// music21's `constants.FlatMask`.
const FLAT_MASK: [(u8, &str); 8] = [
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
const SHARP_MASK: [(u8, &str); 8] = [
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
const ALTERATION_TEXTS: [&str; 6] = ["#", "b", "n", "x", "v", ""];

/// music21's `constants.BarStyles`.
const BAR_STYLES: [&str; 8] = [
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
const DURATION_VALUES: [&str; 7] = ["Whole", "Half", "4th", "8th", "16th", "32nd", "64th"];

/// music21's `constants.MidiInstruments`, the General MIDI names by program.
const MIDI_INSTRUMENTS: [&str; 128] = [
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

/// What the header says that the lines are written from.
struct Header {
    title: Vec<u8>,
    author: Vec<u8>,
    staves: u8,
}

/// One object of a staff, as much of it as is written out.
enum Object {
    /// Read past and written as nothing.
    Silent,
    Clef {
        name: Option<&'static str>,
        shift: Option<&'static str>,
    },
    Key(&'static str),
    Bar(u8),
    Ending(u8),
    TimeSig(i32, i64),
    Tempo(i32),
    Note(Member),
    Rest(String),
    /// A chord's notes; with `rest`, the last group is the rest beside them.
    Chord {
        members: Vec<Member>,
        rest: bool,
    },
    Text(Vec<u8>, i32),
}

/// A note as a chord writes it: its `Dur` and its `Pos`.
#[derive(Clone)]
struct Member {
    duration: String,
    position: String,
}

impl Object {
    /// music21's `dumpMethod`: the text line, or nothing.
    fn line(&self) -> String {
        match self {
            Self::Silent => String::new(),
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
            Self::Note(member) => format!("|Note|Dur:{}|Pos:{}|", member.duration, member.position),
            Self::Rest(duration) => format!("|Rest|Dur:{duration}|"),
            Self::Chord { members, rest } => {
                // Grouped by duration, in the order each first appears.
                let mut groups: Vec<(&str, Vec<&str>)> = Vec::new();
                for member in members {
                    match groups
                        .iter_mut()
                        .find(|(duration, _)| *duration == member.duration)
                    {
                        Some((_, positions)) => positions.push(&member.position),
                        None => groups.push((&member.duration, vec![&member.position])),
                    }
                }
                let mut line = "|Chord".to_string();
                let last = groups.len().saturating_sub(1);
                for (index, (duration, positions)) in groups.iter().enumerate() {
                    let (dur, pos) = if *rest && index == last {
                        ("Dur2", "Pos2")
                    } else {
                        ("Dur", "Pos")
                    };
                    line.push_str(&format!("|{dur}:{duration}|{pos}:{}", positions.join(",")));
                }
                line
            }
            Self::Text(text, position) => {
                format!("|Text|Text:{}|Pos:{position}", latin1(text))
            }
        }
    }
}

/// music21's `NWCConverter`: the file and where it has got to in it.
struct Bytes<'a> {
    data: &'a [u8],
    at: usize,
    version: u32,
    /// The alteration last written on each staff position, by the position
    /// modulo seven, until a barline.
    alterations: Vec<(i32, String)>,
}

impl Bytes<'_> {
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

    /// `parseHeader`, with `fileVersion` read where music21 reads it.
    fn header(&mut self) -> Result<Header> {
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
        self.skip(4);
        let _user = self.until_nul();
        let _unknown = self.until_nul();
        self.skip(10);
        let title = self.until_nul();
        let author = self.until_nul();
        if self.version >= 200 {
            let _lyricist = self.until_nul();
        }
        let _copyright = self.until_nul();
        let _second_copyright = self.until_nul();
        let _comment = self.until_nul();
        let _extend_last_system = self.byte()?;
        let _increase_note_spacing = self.byte()?;
        self.take(5);
        let _measure_numbers = self.byte()?;
        self.take(1);
        let _measure_start = self.short()?;
        if self.version >= 130 {
            let _margins = self.until_nul();
        }
        let _unused = self.byte()?;
        self.take(2);
        if self.version >= 130 {
            self.take(32);
            let _allow_layering = self.byte()?;
        }
        if self.version >= 200 {
            let _typeface = self.until_nul();
        }
        let _staff_height = self.short()?;
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
            let _name = self.until_nul();
            let _style = self.byte()?;
            let _size = self.byte()?;
            let _unused = self.byte()?;
            let _charset = self.byte()?;
        }
        let _title_page = self.byte()?;
        let _staff_labels = self.byte()?;
        let _page_number_start = self.short()?;
        if self.version >= 200 {
            self.skip(1);
        }
        let staves = self.byte()?;
        self.skip(1);
        Ok(Header {
            title,
            author,
            staves,
        })
    }

    /// One staff, `NWCStaff.parse`, written out as `NWCStaff.dump` writes
    /// it.
    fn staff(&mut self) -> Result<Vec<String>> {
        let _name = self.until_nul();
        let mut label = Vec::new();
        let mut instrument = Vec::new();
        if self.version >= 200 {
            label = self.until_nul();
            instrument = self.until_nul();
        }
        let _group = self.until_nul();
        let mut transposition = None;
        let mut lyrics = 0;
        if self.version >= 200 {
            self.skip(27);
            let _lines = self.byte()?;
            let _layer_with_next = self.short()?;
            transposition = Some(self.short()?);
            let _volume = self.short()?;
            let _pan = self.short()?;
            let _color = self.byte()?;
            let _align_syllable = self.short()?;
            lyrics = self.short()?;
        } else if self.version == 175 {
            self.skip(11);
            let patch = usize::from(self.byte()?);
            let index = if patch > 0 && patch < MIDI_INSTRUMENTS.len() {
                patch - 1
            } else {
                0
            };
            instrument = MIDI_INSTRUMENTS[index].bytes().collect();
            self.skip(10);
            transposition = Some(self.signed()?);
            self.skip(6);
            let _align_syllable = self.short()?;
            lyrics = self.short()?;
        }
        if lyrics > 0 {
            let _alignment = self.short()?;
            let _offset = self.short()?;
        }

        // `parseLyrics`: read past, as music21 writes none of them out.
        for _ in 0..lyrics {
            let size = self.short().unwrap_or(0);
            if size > 0 {
                let _length = self.short()?;
                let start = self.at;
                let _junk = self.short()?;
                for _ in 0..1000 {
                    if self.until_nul().is_empty() {
                        break;
                    }
                }
                self.at = start.saturating_add_signed(size as isize);
            }
        }
        if lyrics > 0 {
            let _junk = self.short()?;
        }
        let _junk = self.short()?;

        let mut count = self.short()?;
        if self.version > 150 {
            count -= 2;
        }
        let mut objects = Vec::new();
        for _ in 0..count.max(0) {
            objects.push(self.object()?);
        }

        let instrument = if instrument.is_empty() {
            "Acoustic Grand Piano".to_string()
        } else {
            latin1(&instrument)
        };
        let label = if label.is_empty() {
            instrument.clone()
        } else {
            latin1(&label)
        };
        let mut lines = vec![format!("|AddStaff|Name:{label}")];
        let mut staff_instrument = format!("|StaffInstrument|Name:{instrument}");
        if let Some(patch) = MIDI_INSTRUMENTS.iter().position(|name| *name == instrument) {
            staff_instrument.push_str(&format!("|Patch:{patch}"));
        }
        staff_instrument.push_str(&format!(
            "|Trans:{}",
            transposition.map_or("None".to_string(), |semitones| semitones.to_string())
        ));
        lines.push(staff_instrument);
        lines.extend(
            objects
                .iter()
                .map(Object::line)
                .filter(|line| !line.is_empty()),
        );
        Ok(lines)
    }

    /// `NWCObject.parse`: an object of the type its first short names.
    fn object(&mut self) -> Result<Object> {
        let kind = self.short()?;
        if !(0..19).contains(&kind) {
            return Err(error(format!(
                "Cannot translate objectType: {kind}; max is 19"
            )));
        }
        if self.version >= 170 {
            let _visible = self.byte()?;
        }
        Ok(match kind {
            0 => {
                let clef = self.short()?;
                let shift = self.short()?;
                let clef_count = i32::try_from(CLEF_NAMES.len()).unwrap_or(0);
                let shift_count = i32::try_from(OCTAVE_SHIFT_NAMES.len()).unwrap_or(0);
                Object::Clef {
                    name: if clef < clef_count {
                        Some(
                            python_index(&CLEF_NAMES, clef)
                                .ok_or_else(|| error(format!("no clef numbered {clef}")))?,
                        )
                    } else {
                        None
                    },
                    shift: if shift < shift_count {
                        python_index(&OCTAVE_SHIFT_NAMES, shift)
                            .ok_or_else(|| error(format!("no octave shift numbered {shift}")))?
                    } else {
                        None
                    },
                }
            }
            1 => {
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
                Object::Key(
                    if flats > 0
                        && let Some(text) = mask(&FLAT_MASK, flats)
                    {
                        text
                    } else if sharps > 0
                        && let Some(text) = mask(&SHARP_MASK, sharps)
                    {
                        text
                    } else {
                        ""
                    },
                )
            }
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
                Object::Silent
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
                Object::Silent
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
                let (members, _) = self.chord()?;
                Object::Chord {
                    members,
                    rest: false,
                }
            }
            11 => {
                if self.version >= 170 {
                    self.take(3);
                }
                Object::Silent
            }
            12 => {
                if self.version >= 170 {
                    self.take(2);
                }
                let _style = self.short()?;
                Object::Silent
            }
            13 => {
                self.take(2);
                if self.version == 175 || self.version <= 155 {
                    self.take(32);
                } else {
                    self.take(31);
                }
                Object::Silent
            }
            14 => {
                self.take(4);
                Object::Silent
            }
            15 | 16 => {
                let _position = self.byte()?;
                if self.version >= 170 {
                    let _placement = self.byte()?;
                }
                let _style = self.byte()?;
                Object::Silent
            }
            17 => {
                let position = self.signed()?;
                let _data = self.byte()?;
                let _font = self.byte()?;
                Object::Text(self.until_nul(), position)
            }
            _ => {
                let (mut members, data) = self.chord()?;
                let duration = *data.first().ok_or_else(Self::ended)?;
                members.push(Member {
                    duration: duration_text(duration, &data, None)?,
                    position: "0".to_string(),
                });
                Object::Chord {
                    members,
                    rest: true,
                }
            }
        })
    }

    /// `note`: a note's length, its position on the staff with the
    /// alteration in force there, and whether it is tied on.
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
        if self.version >= 200 && second & 0x40 != 0 {
            let _stem = self.byte()?;
        }
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
            position: format!("{alteration}{position}{tie}"),
        })
    }

    /// `noteChordMember`: the chord's own bytes and its notes, which only a
    /// file of version 1.75 says how many of there are.
    fn chord(&mut self) -> Result<(Vec<Member>, Vec<u8>)> {
        let mut notes = 0;
        let data = if self.version <= 170 {
            self.take(12)
        } else if self.version == 175 {
            let data = self.take(10);
            notes = *data.get(8).ok_or_else(Self::ended)?;
            data
        } else {
            self.take(8)
        };
        if self.version >= 200 && data.get(7).ok_or_else(Self::ended)? & 0x40 != 0 {
            let _stem = self.byte()?;
        }
        let mut members = Vec::new();
        for _ in 0..notes {
            match self.object()? {
                Object::Note(member) => members.push(member),
                // A rest is written at position nought, as music21's
                // object starts out.
                Object::Rest(duration) => members.push(Member {
                    duration,
                    position: "0".to_string(),
                }),
                _ => return Err(error("a chord holding something other than a note")),
            }
        }
        Ok((members, data))
    }
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
            *attributes.first().ok_or_else(Bytes::ended)?,
            attributes.get(1).ok_or_else(Bytes::ended)? & 0x20,
        ),
        None => (*data.get(3).ok_or_else(Bytes::ended)?, 0),
    };
    let triplet = data.get(1).ok_or_else(Bytes::ended)? & 0x0c;
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
        assert_eq!(python_index(&CLEF_NAMES, -1), Some("TenorPercussion"));
        assert_eq!(python_index(&CLEF_NAMES, 4), None);
        assert_eq!(python_index(&CLEF_NAMES, -5), None);
    }

    #[test]
    fn a_chord_groups_its_notes_by_length_and_a_rest_comes_last() {
        let member = |duration: &str, position: &str| Member {
            duration: duration.to_string(),
            position: position.to_string(),
        };
        let chord = Object::Chord {
            members: vec![
                member("4th", "1"),
                member("Half", "3"),
                member("4th", "#5^"),
                member("8th", "0"),
            ],
            rest: true,
        };
        assert_eq!(
            chord.line(),
            "|Chord|Dur:4th|Pos:1,#5^|Dur:Half|Pos:3|Dur2:8th|Pos2:0"
        );
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
