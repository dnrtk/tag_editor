use super::error::{MetadataError, Result};

const SOI: [u8; 2] = [0xFF, 0xD8];
const APP1_MARKER: u8 = 0xE1;
const XMP_SIG: &[u8] = b"http://ns.adobe.com/xap/1.0/\0";

pub fn read_xmp(data: &[u8]) -> Option<String> {
    for seg in iter_segments(data) {
        if seg.marker == APP1_MARKER {
            if let Some(rest) = seg.payload.strip_prefix(XMP_SIG) {
                return std::str::from_utf8(rest).ok().map(str::to_owned);
            }
        }
    }
    None
}

pub fn write_xmp(data: &[u8], xmp: &[u8]) -> Result<Vec<u8>> {
    if !is_jpeg(data) {
        return Err(MetadataError::Malformed("not a JPEG"));
    }

    let new_segment = build_app1(xmp);

    // Replace existing XMP APP1 segment if present.
    for seg in iter_segments(data) {
        if seg.marker == APP1_MARKER && seg.payload.starts_with(XMP_SIG) {
            let mut out = Vec::with_capacity(data.len() + new_segment.len());
            out.extend_from_slice(&data[..seg.start]);
            out.extend_from_slice(&new_segment);
            out.extend_from_slice(&data[seg.end..]);
            return Ok(out);
        }
    }

    // No existing XMP — insert immediately after SOI.
    let mut out = Vec::with_capacity(data.len() + new_segment.len());
    out.extend_from_slice(&data[..2]);
    out.extend_from_slice(&new_segment);
    out.extend_from_slice(&data[2..]);
    Ok(out)
}

fn is_jpeg(data: &[u8]) -> bool {
    data.len() >= 2 && data[..2] == SOI
}

fn build_app1(xmp: &[u8]) -> Vec<u8> {
    let payload_len = XMP_SIG.len() + xmp.len();
    let segment_len = (payload_len + 2) as u16; // includes the 2-byte length itself

    let mut seg = Vec::with_capacity(payload_len + 4);
    seg.push(0xFF);
    seg.push(APP1_MARKER);
    seg.extend_from_slice(&segment_len.to_be_bytes());
    seg.extend_from_slice(XMP_SIG);
    seg.extend_from_slice(xmp);
    seg
}

struct Segment<'a> {
    marker: u8,
    payload: &'a [u8],
    /// Byte offset where 0xFF marker prefix begins.
    start: usize,
    /// One past the last byte of the segment.
    end: usize,
}

fn iter_segments(data: &[u8]) -> SegmentIter<'_> {
    SegmentIter {
        data,
        pos: if is_jpeg(data) { 2 } else { data.len() },
    }
}

struct SegmentIter<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Iterator for SegmentIter<'a> {
    type Item = Segment<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos + 4 > self.data.len() || self.data[self.pos] != 0xFF {
            return None;
        }
        let marker = self.data[self.pos + 1];
        let len = u16::from_be_bytes([self.data[self.pos + 2], self.data[self.pos + 3]]) as usize;
        let payload_start = self.pos + 4;
        let payload_end = self.pos + 2 + len;
        if payload_end > self.data.len() {
            return None;
        }
        let seg = Segment {
            marker,
            payload: &self.data[payload_start..payload_end],
            start: self.pos,
            end: payload_end,
        };
        self.pos = payload_end;
        Some(seg)
    }
}
