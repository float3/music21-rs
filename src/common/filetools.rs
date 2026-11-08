//! music21's `common.fileTools`, as far as it is not about Python objects.

use std::io::{self, Read};

/// The most bytes music21 will uncompress from one member of a compressed
/// file: 256 MiB.
pub const MAX_UNCOMPRESSED_BYTES: u64 = 256 * 1024 * 1024;

/// How much is read at a time.
const CHUNK: usize = 1024 * 1024;

/// Everything a reader holds, read a mebibyte at a time, or `None` as soon
/// as it has given more than `limit` bytes. music21's `readZipMember` reads
/// a member so, rather than trusting the size its header claims.
///
/// ```
/// use music21::common::filetools::read_limited;
///
/// assert_eq!(read_limited(&b"<score/>"[..], 8).unwrap(), Some(b"<score/>".to_vec()));
/// assert_eq!(read_limited(&b"<score/>"[..], 7).unwrap(), None);
/// ```
pub fn read_limited<R: Read>(mut reader: R, limit: u64) -> io::Result<Option<Vec<u8>>> {
    let mut read = Vec::new();
    let mut chunk = vec![0; CHUNK];
    loop {
        let count = reader.read(&mut chunk)?;
        if count == 0 {
            return Ok(Some(read));
        }
        read.extend_from_slice(&chunk[..count]);
        if read.len() as u64 > limit {
            return Ok(None);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A reader longer than one chunk is read whole, and stops being read
    /// once past the limit.
    #[test]
    fn long_readers_are_read_whole_or_refused() {
        let long = vec![7u8; CHUNK * 2 + 5];
        assert_eq!(
            read_limited(&long[..], MAX_UNCOMPRESSED_BYTES).unwrap(),
            Some(long.clone())
        );
        assert_eq!(read_limited(&long[..], CHUNK as u64).unwrap(), None);
        assert_eq!(read_limited(&b""[..], 0).unwrap(), Some(Vec::new()));
    }
}
