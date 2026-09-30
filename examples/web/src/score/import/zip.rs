//! Just enough of the zip format to open what Guitar Pro 7 and compressed
//! MusicXML pack their scores in: the central directory, and files stored or
//! deflated.

const DAMAGED: &str = "the zip archive is damaged";

fn u16_at(bytes: &[u8], at: usize) -> Result<usize, String> {
    let pair = bytes.get(at..at + 2).ok_or(DAMAGED)?;
    Ok(u16::from_le_bytes([pair[0], pair[1]]) as usize)
}

fn u32_at(bytes: &[u8], at: usize) -> Result<usize, String> {
    let quad = bytes.get(at..at + 4).ok_or(DAMAGED)?;
    Ok(u32::from_le_bytes([quad[0], quad[1], quad[2], quad[3]]) as usize)
}

struct Entry {
    name: String,
    method: usize,
    compressed: usize,
    header: usize,
}

pub(super) struct Archive<'a> {
    bytes: &'a [u8],
    entries: Vec<Entry>,
}

impl<'a> Archive<'a> {
    pub(super) fn new(bytes: &'a [u8]) -> Result<Self, String> {
        // The directory's closing record is the last thing in the file but
        // for a comment of at most 64 KiB.
        let last = bytes.len().checked_sub(22).ok_or(DAMAGED)?;
        let end = (last.saturating_sub(65_535)..=last)
            .rev()
            .find(|&at| bytes[at..].starts_with(b"PK\x05\x06"))
            .ok_or(DAMAGED)?;
        let count = u16_at(bytes, end + 10)?;
        let mut at = u32_at(bytes, end + 16)?;
        let mut entries = Vec::with_capacity(count);
        for _ in 0..count {
            if !bytes
                .get(at..)
                .is_some_and(|rest| rest.starts_with(b"PK\x01\x02"))
            {
                return Err(DAMAGED.to_string());
            }
            let name_length = u16_at(bytes, at + 28)?;
            let name = bytes.get(at + 46..at + 46 + name_length).ok_or(DAMAGED)?;
            entries.push(Entry {
                name: String::from_utf8_lossy(name).into_owned(),
                method: u16_at(bytes, at + 10)?,
                compressed: u32_at(bytes, at + 20)?,
                header: u32_at(bytes, at + 42)?,
            });
            at += 46 + name_length + u16_at(bytes, at + 30)? + u16_at(bytes, at + 32)?;
        }
        Ok(Self { bytes, entries })
    }

    pub(super) fn names(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|entry| entry.name.as_str())
    }

    /// The file called `name`, or None where the archive has no such file.
    pub(super) fn read(&self, name: &str) -> Result<Option<Vec<u8>>, String> {
        let Some(entry) = self.entries.iter().find(|entry| entry.name == name) else {
            return Ok(None);
        };
        let at = entry.header;
        if !self
            .bytes
            .get(at..)
            .is_some_and(|rest| rest.starts_with(b"PK\x03\x04"))
        {
            return Err(DAMAGED.to_string());
        }
        let start = at + 30 + u16_at(self.bytes, at + 26)? + u16_at(self.bytes, at + 28)?;
        let data = self
            .bytes
            .get(start..start + entry.compressed)
            .ok_or(DAMAGED)?;
        match entry.method {
            0 => Ok(Some(data.to_vec())),
            8 => miniz_oxide::inflate::decompress_to_vec(data)
                .map(Some)
                .map_err(|_| format!("{name} in the zip archive could not be inflated")),
            method => Err(format!(
                "{name} is compressed with zip method {method}, which this reader does not know"
            )),
        }
    }
}
