use std::io::{Read, Seek, SeekFrom};

use super::error::{MetadataError, Result};

const RIFF: &[u8; 4] = b"RIFF";
const WEBP: &[u8; 4] = b"WEBP";
const XMP_ID: [u8; 4] = *b"XMP ";

/// Reads the XMP packet by walking RIFF chunk headers only. The `XMP ` chunk's
/// data is read; every other chunk (including the large `VP8`/`VP8L` image data)
/// is skipped with a seek, so the image body is never pulled into memory.
pub fn read_xmp_streaming<R: Read + Seek>(r: &mut R) -> Option<String> {
    let mut riff = [0u8; 12];
    r.read_exact(&mut riff).ok()?;
    if &riff[0..4] != RIFF || &riff[8..12] != WEBP {
        return None;
    }

    let mut header = [0u8; 8];
    loop {
        if r.read_exact(&mut header).is_err() {
            return None;
        }
        let id: [u8; 4] = header[0..4].try_into().ok()?;
        let size = u32::from_le_bytes(header[4..8].try_into().ok()?) as usize;
        // RIFF chunks are padded to an even byte boundary.
        let padded = size + (size & 1);

        if id == XMP_ID {
            let mut data = vec![0u8; size];
            r.read_exact(&mut data).ok()?;
            return std::str::from_utf8(&data).ok().map(str::to_owned);
        }
        r.seek(SeekFrom::Current(padded as i64)).ok()?;
    }
}

pub fn write_xmp(data: &[u8], xmp: &[u8]) -> Result<Vec<u8>> {
    if !is_webp(data) {
        return Err(MetadataError::Malformed("not a WebP"));
    }

    let chunks: Vec<Chunk> = iter_chunks(data).collect();

    let mut out = Vec::with_capacity(data.len() + xmp.len() + 16);
    out.extend_from_slice(RIFF);
    out.extend_from_slice(&[0u8; 4]); // RIFF size — patched at the end.
    out.extend_from_slice(WEBP);

    let mut replaced = false;
    for chunk in &chunks {
        if chunk.id == XMP_ID {
            write_chunk(&mut out, &XMP_ID, xmp);
            replaced = true;
        } else {
            write_chunk(&mut out, &chunk.id, chunk.data);
        }
    }
    if !replaced {
        write_chunk(&mut out, &XMP_ID, xmp);
    }

    let riff_size = (out.len() - 8) as u32;
    out[4..8].copy_from_slice(&riff_size.to_le_bytes());
    Ok(out)
}

fn is_webp(data: &[u8]) -> bool {
    data.len() >= 12 && &data[..4] == RIFF && &data[8..12] == WEBP
}

fn write_chunk(out: &mut Vec<u8>, id: &[u8; 4], payload: &[u8]) {
    out.extend_from_slice(id);
    out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    out.extend_from_slice(payload);
    if payload.len() & 1 == 1 {
        out.push(0); // RIFF chunks must be aligned to even byte boundary.
    }
}

struct Chunk<'a> {
    id: [u8; 4],
    data: &'a [u8],
}

fn iter_chunks(data: &[u8]) -> ChunkIter<'_> {
    ChunkIter {
        data,
        pos: if is_webp(data) { 12 } else { data.len() },
    }
}

struct ChunkIter<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Iterator for ChunkIter<'a> {
    type Item = Chunk<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos + 8 > self.data.len() {
            return None;
        }
        let id: [u8; 4] = self.data[self.pos..self.pos + 4].try_into().ok()?;
        let size = u32::from_le_bytes(self.data[self.pos + 4..self.pos + 8].try_into().ok()?) as usize;
        let data_start = self.pos + 8;
        let data_end = data_start.checked_add(size)?;
        if data_end > self.data.len() {
            return None;
        }
        let chunk = Chunk {
            id,
            data: &self.data[data_start..data_end],
        };
        self.pos = data_end + (size & 1);
        Some(chunk)
    }
}
