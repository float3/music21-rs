//! Guitar Pro 3, 4 and 5 (`.gp3`, `.gp4`, `.gp5`): one binary stream of the
//! song's details, its measure headers, its tracks and then every track's
//! beats, measure by measure. The layout followed is the one alphaTab and
//! PyGuitarPro read; what this editor has no use for is skipped by length.

use super::{Beat, Imported, KeySig, Measure, Note, Part, latin1, tuplet_ratio, written_ticks};

const ENDS_EARLY: &str = "the Guitar Pro file ends early";

struct Reader<'a> {
    data: &'a [u8],
    at: usize,
    /// 300 for 3.00, 406 for 4.06, 510 for 5.10.
    version: u32,
}

impl<'a> Reader<'a> {
    fn take(&mut self, count: usize) -> Result<&'a [u8], String> {
        let end = self.at.checked_add(count).ok_or(ENDS_EARLY)?;
        let bytes = self.data.get(self.at..end).ok_or(ENDS_EARLY)?;
        self.at = end;
        Ok(bytes)
    }

    fn skip(&mut self, count: usize) -> Result<(), String> {
        self.take(count).map(|_| ())
    }

    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }

    fn i8(&mut self) -> Result<i8, String> {
        Ok(self.u8()? as i8)
    }

    fn i16(&mut self) -> Result<i16, String> {
        let bytes = self.take(2)?;
        Ok(i16::from_le_bytes([bytes[0], bytes[1]]))
    }

    fn i32(&mut self) -> Result<i32, String> {
        let bytes = self.take(4)?;
        Ok(i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    /// A count read as a signed integer, refused past `limit`.
    fn count(&mut self, limit: i32) -> Result<usize, String> {
        let count = self.i32()?;
        if !(0..=limit).contains(&count) {
            return Err("the Guitar Pro file is damaged".to_string());
        }
        Ok(count as usize)
    }

    /// A string of at most `size` bytes stored in `size` bytes after its length.
    fn fixed_string(&mut self, size: usize) -> Result<String, String> {
        let length = self.u8()? as usize;
        let bytes = self.take(size)?;
        Ok(latin1(&bytes[..length.min(size)]))
    }

    /// A string stored after its length as an integer.
    fn int_string(&mut self) -> Result<String, String> {
        let length = self.count(1 << 20)?;
        Ok(latin1(self.take(length)?))
    }

    /// A string stored after an integer and then its length as a byte.
    fn int_byte_string(&mut self) -> Result<String, String> {
        self.skip(4)?;
        let length = self.u8()? as usize;
        Ok(latin1(self.take(length)?))
    }
}

struct Header {
    meter: (u32, u32),
    key: Option<KeySig>,
    repeat_start: bool,
    repeat_end: u32,
    /// The endings as a mask: bit 0 is the first.
    endings: u8,
    marker: Option<String>,
}

struct Track {
    name: String,
    drums: bool,
    /// Open strings, highest first, as the file numbers them.
    strings: Vec<i32>,
    program: Option<u8>,
}

pub(super) fn read(data: &[u8]) -> Result<Imported, String> {
    let mut reader = Reader {
        data,
        at: 0,
        version: 0,
    };
    let banner = reader.fixed_string(30)?;
    reader.version = banner
        .split('v')
        .next_back()
        .and_then(|number| {
            let (major, minor) = number.split_once('.')?;
            Some(major.trim().parse::<u32>().ok()? * 100 + minor.trim().parse::<u32>().ok()?)
        })
        .filter(|version| (300..600).contains(version))
        .ok_or_else(|| format!("{banner} is a Guitar Pro version this reader does not know"))?;
    let version = reader.version;

    let title = reader.int_byte_string()?;
    let fields = if version >= 500 { 8 } else { 7 };
    for _ in 0..fields {
        reader.int_byte_string()?;
    }
    for _ in 0..reader.count(1000)? {
        reader.int_byte_string()?;
    }
    if version < 500 {
        reader.skip(1)?; // triplet feel
    }
    if version >= 400 {
        reader.skip(4)?; // the track the lyrics belong to
        for _ in 0..5 {
            reader.skip(4)?;
            reader.int_string()?;
        }
    }
    if version >= 510 {
        reader.skip(19)?; // master effects
    }
    if version >= 500 {
        reader.skip(30)?; // page size and margins
        for _ in 0..10 {
            reader.int_byte_string()?; // header and footer lines
        }
        reader.int_byte_string()?; // tempo name
    }
    let tempo = reader.i32()?;
    if version >= 510 {
        reader.skip(1)?;
    }
    let song_key = if version >= 500 {
        let key = reader.i8()? as i32;
        reader.skip(4)?;
        key
    } else {
        let key = reader.i32()?;
        if version >= 400 {
            reader.skip(1)?;
        }
        key
    };
    let mut programs = [0_u8; 64];
    for program in &mut programs {
        *program = reader.i32()?.clamp(0, 127) as u8;
        reader.skip(8)?;
    }
    if version >= 500 {
        reader.skip(42)?; // directions and reverb
    }
    let measure_count = reader.count(100_000)?;
    let track_count = reader.count(128)?;

    let mut headers: Vec<Header> = Vec::with_capacity(measure_count);
    for index in 0..measure_count {
        let header = read_header(&mut reader, index, &headers)?;
        headers.push(header);
    }
    let mut tracks = Vec::with_capacity(track_count);
    for index in 0..track_count {
        tracks.push(read_track(&mut reader, index, &programs)?);
    }
    if version >= 500 {
        reader.skip(if version == 500 { 2 } else { 1 })?;
    }

    let mut parts: Vec<Part> = tracks
        .iter()
        .map(|track| Part {
            name: track.name.clone(),
            program: track.program,
            tuning: track.strings.iter().rev().copied().collect(),
            ..Part::default()
        })
        .collect();
    let mut last_frets = vec![[0_i32; 7]; track_count];
    for (index, header) in headers.iter().enumerate() {
        let mut key = header.key;
        if index == 0 && key.is_none() && (-7..=7).contains(&song_key) {
            key = Some(KeySig {
                sharps: song_key,
                minor: false,
            });
        }
        let frame = Measure {
            meter: Some(header.meter),
            key,
            repeat_start: header.repeat_start,
            repeat_end: header.repeat_end,
            ending: (0..8)
                .filter(|bit| header.endings & (1 << bit) != 0)
                .map(|bit| bit + 1)
                .collect(),
            marker: header.marker.clone(),
            voices: Vec::new(),
        };
        for (t, track) in tracks.iter().enumerate() {
            let mut measure = frame.clone();
            let voices = if version >= 500 { 2 } else { 1 };
            for _ in 0..voices {
                let count = reader.count(1000)?;
                let mut beats = Vec::with_capacity(count);
                for _ in 0..count {
                    read_beat(&mut reader, track, &mut last_frets[t], &mut beats)?;
                }
                measure.voices.push(beats);
            }
            // A line break, which the last measure of a file may leave out.
            if version >= 500 && reader.at < reader.data.len() {
                reader.skip(1)?;
            }
            parts[t].measures.push(measure);
        }
    }

    let mut imported = Imported {
        title,
        tempo: (tempo > 0).then_some(tempo as f64),
        ..Imported::default()
    };
    for (track, part) in tracks.iter().zip(parts) {
        if track.drums {
            imported.skipped.push(format!("{}: drums", track.name));
        } else {
            imported.parts.push(part);
        }
    }
    Ok(imported)
}

fn read_header(reader: &mut Reader<'_>, index: usize, before: &[Header]) -> Result<Header, String> {
    let version = reader.version;
    if version >= 500 && index > 0 {
        reader.skip(1)?;
    }
    let flags = reader.u8()?;
    let previous = before.last();
    let mut meter = previous.map_or((4, 4), |header| header.meter);
    if flags & 0x01 != 0 {
        meter.0 = reader.i8()?.max(1) as u32;
    }
    if flags & 0x02 != 0 {
        meter.1 = reader.i8()?.max(1) as u32;
    }
    let mut header = Header {
        meter,
        key: None,
        repeat_start: flags & 0x04 != 0,
        repeat_end: 0,
        endings: 0,
        marker: None,
    };
    if flags & 0x08 != 0 {
        let count = reader.i8()? as i32;
        // Guitar Pro 5 stores how often the passage is played, the earlier
        // versions how often it is repeated.
        header.repeat_end = (if version >= 500 { count } else { count + 1 }).max(2) as u32;
    }
    if flags & 0x10 != 0 && version < 500 {
        // The earlier versions store the highest ending this bar belongs to
        // and leave out those an earlier bar of the same repeat took.
        let highest = reader.u8()?;
        let mut taken = 0_u8;
        for (at, earlier) in before.iter().enumerate().rev() {
            if earlier.repeat_end > 0 && at + 1 < before.len() || earlier.repeat_start {
                break;
            }
            taken |= earlier.endings;
        }
        header.endings = (0..8_u8)
            .filter(|&bit| highest > bit && taken & (1 << bit) == 0)
            .fold(0, |mask, bit| mask | 1 << bit);
    }
    if flags & 0x20 != 0 {
        header.marker = Some(reader.int_byte_string()?).filter(|name| !name.trim().is_empty());
        reader.skip(4)?; // colour
    }
    if flags & 0x40 != 0 {
        let sharps = reader.i8()? as i32;
        let minor = reader.i8()? != 0;
        header.key = Some(KeySig { sharps, minor });
    }
    if version >= 500 {
        if flags & 0x10 != 0 {
            header.endings = reader.u8()?;
        }
        if flags & 0x03 != 0 {
            reader.skip(4)?; // beaming
        }
        if flags & 0x10 == 0 {
            reader.skip(1)?;
        }
        reader.skip(1)?; // triplet feel
    }
    Ok(header)
}

fn read_track(reader: &mut Reader<'_>, index: usize, programs: &[u8; 64]) -> Result<Track, String> {
    let version = reader.version;
    if version >= 500 && (index == 0 || version == 500) {
        reader.skip(1)?;
    }
    let flags = reader.u8()?;
    let name = reader.fixed_string(40)?;
    let string_count = reader.count(7)?;
    let mut strings = Vec::with_capacity(string_count);
    for string in 0..7 {
        let tuning = reader.i32()?;
        if string < string_count {
            strings.push(tuning);
        }
    }
    let port = reader.i32()?;
    let channel = reader.i32()?;
    reader.skip(4 + 4 + 4 + 4)?; // effect channel, frets, capo, colour
    if version >= 500 {
        reader.skip(2 + 1 + 1 + 1 + 12 + 12 + 12)?;
        if version == 500 {
            reader.skip(3)?;
        } else {
            reader.skip(4 + 4)?;
            reader.int_byte_string()?;
            reader.int_byte_string()?;
        }
    }
    let slot = (port - 1) * 16 + channel - 1;
    let program = usize::try_from(slot)
        .ok()
        .and_then(|slot| programs.get(slot))
        .or_else(|| {
            usize::try_from(channel - 1)
                .ok()
                .and_then(|slot| programs.get(slot))
        })
        .copied();
    Ok(Track {
        name: name.trim().to_string(),
        drums: flags & 0x01 != 0 || channel == 10,
        strings,
        program,
    })
}

fn read_beat(
    reader: &mut Reader<'_>,
    track: &Track,
    last_frets: &mut [i32; 7],
    beats: &mut Vec<Beat>,
) -> Result<(), String> {
    let version = reader.version;
    let flags = reader.u8()?;
    if flags & 0x40 != 0 {
        reader.skip(1)?; // empty or rest
    }
    let value = reader.i8()?.clamp(-2, 5) as i32;
    let mut beat = Beat {
        written: written_ticks(1 << (value + 2), u32::from(flags & 0x01 != 0)),
        ..Beat::default()
    };
    if flags & 0x20 != 0 {
        beat.tuplet = tuplet_ratio(reader.i32()?.max(0) as u32);
    }
    if flags & 0x02 != 0 {
        beat.chord = read_chord(reader)?.filter(|name| !name.trim().is_empty());
    }
    if flags & 0x04 != 0 {
        beat.text = Some(reader.int_byte_string()?).filter(|text| !text.trim().is_empty());
    }
    if flags & 0x08 != 0 {
        read_beat_effects(reader)?;
    }
    if flags & 0x10 != 0 {
        read_mix_table(reader)?;
    }
    let string_flags = reader.u8()?;
    let mut graces = Vec::new();
    for string in 1..=track.strings.len() {
        if string_flags & (1 << (7 - string)) == 0 {
            continue;
        }
        let open = track.strings[string - 1];
        let NoteRead { fret, kind, grace } = read_note(reader)?;
        if let Some(grace) = grace {
            graces.push((open + grace.0, grace.1));
        }
        let fret = if kind == 2 {
            last_frets[string - 1]
        } else {
            fret
        };
        last_frets[string - 1] = fret;
        if kind != 3 {
            beat.notes.push(Note {
                midi: open + fret,
                tied: kind == 2,
            });
        }
    }
    if version >= 500 {
        let more = reader.i16()?;
        if more & 0x0800 != 0 {
            reader.skip(1)?;
        }
    }
    for (midi, written) in graces {
        beats.push(Beat {
            written,
            grace: true,
            notes: vec![Note { midi, tied: false }],
            ..Beat::default()
        });
    }
    beats.push(beat);
    Ok(())
}

/// A note as the file writes it.
struct NoteRead {
    fret: i32,
    /// 1 plain, 2 held over from the note before, 3 dead.
    kind: u8,
    /// A grace note before it: its fret and written length.
    grace: Option<(i32, i64)>,
}

fn read_note(reader: &mut Reader<'_>) -> Result<NoteRead, String> {
    let version = reader.version;
    let flags = reader.u8()?;
    let kind = if flags & 0x20 != 0 { reader.u8()? } else { 1 };
    if flags & 0x01 != 0 && version < 500 {
        reader.skip(2)?;
    }
    if flags & 0x10 != 0 {
        reader.skip(1)?; // loudness
    }
    let fret = if flags & 0x20 != 0 {
        reader.i8()?.max(0) as i32
    } else {
        0
    };
    if flags & 0x80 != 0 {
        reader.skip(2)?; // fingering
    }
    if version >= 500 {
        if flags & 0x01 != 0 {
            reader.skip(8)?;
        }
        reader.skip(1)?;
    }
    let grace = if flags & 0x08 != 0 {
        read_note_effects(reader)?
    } else {
        None
    };
    Ok(NoteRead { fret, kind, grace })
}

fn read_bend(reader: &mut Reader<'_>) -> Result<(), String> {
    reader.skip(5)?;
    let points = reader.count(1000)?;
    reader.skip(points * 9)
}

fn read_note_effects(reader: &mut Reader<'_>) -> Result<Option<(i32, i64)>, String> {
    let version = reader.version;
    let first = reader.u8()?;
    let second = if version >= 400 { reader.u8()? } else { 0 };
    if first & 0x01 != 0 {
        read_bend(reader)?;
    }
    let mut grace = None;
    if first & 0x10 != 0 {
        // Fret, loudness, transition and length, in every version; the
        // length is read as a 32nd, as alphaTab reads it and as Guitar Pro 7
        // writes the same grace notes.
        let fret = reader.u8()? as i32;
        reader.skip(if version >= 500 { 4 } else { 3 })?;
        grace = Some((fret, written_ticks(32, 0)));
    }
    if second & 0x04 != 0 {
        reader.skip(1)?; // tremolo picking
    }
    if second & 0x08 != 0 {
        reader.skip(1)?; // slide
    }
    if second & 0x10 != 0 {
        let kind = reader.i8()?;
        if version >= 500 {
            match kind {
                2 => reader.skip(3)?,
                3 => reader.skip(1)?,
                _ => {}
            }
        }
    }
    if second & 0x20 != 0 {
        reader.skip(2)?; // trill
    }
    Ok(grace)
}

fn read_beat_effects(reader: &mut Reader<'_>) -> Result<(), String> {
    if reader.version < 400 {
        let flags = reader.u8()?;
        if flags & 0x20 != 0 {
            reader.skip(1 + 4)?;
        }
        if flags & 0x40 != 0 {
            reader.skip(2)?;
        }
        return Ok(());
    }
    let first = reader.u8()?;
    let second = reader.u8()?;
    if first & 0x20 != 0 {
        reader.skip(1)?;
    }
    if second & 0x04 != 0 {
        read_bend(reader)?;
    }
    if first & 0x40 != 0 {
        reader.skip(2)?;
    }
    if second & 0x02 != 0 {
        reader.skip(1)?;
    }
    Ok(())
}

fn read_mix_table(reader: &mut Reader<'_>) -> Result<(), String> {
    let version = reader.version;
    reader.skip(1)?; // instrument
    if version >= 500 {
        reader.skip(16)?;
    }
    let mut changes = [0_i8; 6];
    for change in &mut changes {
        *change = reader.i8()?;
    }
    if version >= 500 {
        reader.int_byte_string()?;
    }
    let tempo = reader.i32()?;
    for change in changes {
        if change >= 0 {
            reader.skip(1)?;
        }
    }
    if tempo >= 0 {
        reader.skip(if version >= 510 { 2 } else { 1 })?;
    }
    if version >= 400 {
        reader.skip(1)?;
    }
    if version >= 500 {
        reader.skip(1)?;
    }
    if version >= 510 {
        reader.int_byte_string()?;
        reader.int_byte_string()?;
    }
    Ok(())
}

/// A chord diagram, of which only the name is kept.
fn read_chord(reader: &mut Reader<'_>) -> Result<Option<String>, String> {
    let version = reader.version;
    if version >= 500 {
        reader.skip(17)?;
        let name = reader.fixed_string(21)?;
        reader.skip(4 + 4 + 28 + 32)?;
        return Ok(Some(name));
    }
    if reader.u8()? != 0 {
        if version >= 400 {
            reader.skip(16)?;
            let name = reader.fixed_string(21)?;
            reader.skip(4 + 4 + 28 + 32)?;
            return Ok(Some(name));
        }
        reader.skip(25)?;
        let name = reader.fixed_string(34)?;
        reader.skip(4 + 24 + 36)?;
        return Ok(Some(name));
    }
    let name = reader.int_byte_string()?;
    let first_fret = reader.i32()?;
    if first_fret > 0 {
        reader.skip(if version >= 406 { 7 } else { 6 } * 4)?;
    }
    Ok(Some(name))
}
