use super::error::{MetadataError, Result};
use crc32fast::Hasher;

const SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
const XMP_KEYWORD: &[u8] = b"XML:com.adobe.xmp";
const ITXT: [u8; 4] = *b"iTXt";
const IEND: [u8; 4] = *b"IEND";

pub fn read_xmp(data: &[u8]) -> Option<String> {
    for chunk in iter_chunks(data) {
        if chunk.kind == ITXT && is_xmp_itxt(chunk.data) {
            return parse_itxt_text(chunk.data).map(str::to_owned);
        }
    }
    None
}

pub fn write_xmp(data: &[u8], xmp: &[u8]) -> Result<Vec<u8>> {
    if !is_png(data) {
        return Err(MetadataError::Malformed("not a PNG"));
    }

    let new_chunk = build_itxt_chunk(XMP_KEYWORD, xmp);

    let mut out = Vec::with_capacity(data.len() + new_chunk.len());
    out.extend_from_slice(SIGNATURE);

    let mut replaced = false;
    let mut iend_seen = false;
    for chunk in iter_chunks(data) {
        if chunk.kind == IEND {
            // Insert XMP before IEND if not yet replaced.
            if !replaced {
                out.extend_from_slice(&new_chunk);
            }
            out.extend_from_slice(&data[chunk.start..chunk.end]);
            iend_seen = true;
            break;
        }
        if !replaced && chunk.kind == ITXT && is_xmp_itxt(chunk.data) {
            out.extend_from_slice(&new_chunk);
            replaced = true;
        } else {
            out.extend_from_slice(&data[chunk.start..chunk.end]);
        }
    }

    if !iend_seen {
        return Err(MetadataError::Malformed("missing IEND chunk"));
    }
    Ok(out)
}

fn is_png(data: &[u8]) -> bool {
    data.len() >= SIGNATURE.len() && &data[..SIGNATURE.len()] == SIGNATURE
}

fn is_xmp_itxt(chunk_data: &[u8]) -> bool {
    let zero = chunk_data.iter().position(|&b| b == 0);
    matches!(zero, Some(end) if chunk_data[..end].eq_ignore_ascii_case(XMP_KEYWORD))
}

/// Extract the text portion of an iTXt chunk: skips keyword, flags, language tag,
/// and translated keyword. See PNG spec §11.3.4.
fn parse_itxt_text(chunk_data: &[u8]) -> Option<&str> {
    let kw_end = chunk_data.iter().position(|&b| b == 0)?;
    let mut p = kw_end + 1 + 2; // skip null + compression flag + compression method
    if p >= chunk_data.len() {
        return None;
    }
    p += chunk_data[p..].iter().position(|&b| b == 0)? + 1; // skip language tag
    if p >= chunk_data.len() {
        return None;
    }
    p += chunk_data[p..].iter().position(|&b| b == 0)? + 1; // skip translated keyword
    if p > chunk_data.len() {
        return None;
    }
    std::str::from_utf8(&chunk_data[p..]).ok()
}

fn build_itxt_chunk(keyword: &[u8], text: &[u8]) -> Vec<u8> {
    let mut payload = Vec::with_capacity(keyword.len() + text.len() + 5);
    payload.extend_from_slice(keyword);
    payload.push(0); // null terminator for keyword
    payload.push(0); // compression flag (uncompressed)
    payload.push(0); // compression method
    payload.push(0); // empty language tag, null-terminated
    payload.push(0); // empty translated keyword, null-terminated
    payload.extend_from_slice(text);

    let mut chunk = Vec::with_capacity(payload.len() + 12);
    chunk.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    chunk.extend_from_slice(&ITXT);
    chunk.extend_from_slice(&payload);

    let mut hasher = Hasher::new();
    hasher.update(&ITXT);
    hasher.update(&payload);
    chunk.extend_from_slice(&hasher.finalize().to_be_bytes());
    chunk
}

struct Chunk<'a> {
    kind: [u8; 4],
    data: &'a [u8],
    /// Byte offset of the chunk's length field.
    start: usize,
    /// One past the last byte of the chunk's CRC.
    end: usize,
}

fn iter_chunks(data: &[u8]) -> ChunkIter<'_> {
    ChunkIter {
        data,
        pos: if is_png(data) { SIGNATURE.len() } else { data.len() },
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
        let len = u32::from_be_bytes(self.data[self.pos..self.pos + 4].try_into().ok()?) as usize;
        let kind: [u8; 4] = self.data[self.pos + 4..self.pos + 8].try_into().ok()?;
        let data_start = self.pos + 8;
        let data_end = data_start.checked_add(len)?;
        let chunk_end = data_end.checked_add(4)?;
        if chunk_end > self.data.len() {
            return None;
        }
        let chunk = Chunk {
            kind,
            data: &self.data[data_start..data_end],
            start: self.pos,
            end: chunk_end,
        };
        self.pos = chunk_end;
        Some(chunk)
    }
}
