use crc32fast::Hasher as Crc32Hasher;
use quick_xml::Reader;
use quick_xml::events::Event;
use std::collections::HashSet;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

/// タグを読み込む (XMP dc:subject の rdf:li を使う)
pub fn load_tags(image_path: &Path) -> Vec<String> {
    if !is_supported_format(image_path) {
        return Vec::new();
    }

    if let Ok(mut f) = fs::File::open(image_path) {
        let mut buf = Vec::new();
        if f.read_to_end(&mut buf).is_ok() {
            if let Some(xmp) = extract_xmp_from_bytes(&buf) {
                return parse_xmp_subjects(&xmp);
            }
        }
    }

    Vec::new()
}

/// タグを保存する (XMP dc:subject の rdf:li に保存)
pub fn save_tags(image_path: &Path, tags: &[String]) -> std::io::Result<()> {
    if !is_supported_format(image_path) {
        return Err(std::io::Error::new(std::io::ErrorKind::Unsupported, "Unsupported format"));
    }

    let xmp = build_xmp_packet(tags);

    let mut data = fs::read(image_path)?;

    if is_jpeg(image_path) {
        data = write_xmp_to_jpeg_bytes(&data, xmp.as_bytes());
    } else if is_png(image_path) {
        data = write_xmp_to_png_bytes(&data, xmp.as_bytes())?;
    } else if image_path.extension().and_then(|e| e.to_str()).map(|s| s.eq_ignore_ascii_case("webp")).unwrap_or(false) {
        data = write_xmp_to_webp_bytes(&data, xmp.as_bytes())?;
    } else {
        // fallback: overwrite file with same data
    }

    let mut f = fs::File::create(image_path)?;
    f.write_all(&data)?;
    Ok(())
}

/// タグの追加
pub fn add_tag(tags: &mut Vec<String>, tag: &str) {
    let tag = tag.trim().to_string();
    if !tag.is_empty() && !tags.contains(&tag) {
        tags.push(tag);
    }
}

/// タグの削除
pub fn remove_tag(tags: &mut Vec<String>, tag: &str) {
    tags.retain(|t| t != tag);
}

/// タグのトグル（存在すれば削除、なければ追加）
pub fn toggle_tag(tags: &mut Vec<String>, tag: &str) -> bool {
    let tag = tag.trim().to_string();
    if tags.contains(&tag) {
        remove_tag(tags, &tag);
        false
    } else {
        add_tag(tags, &tag);
        true
    }
}

/// ディレクトリ内の全画像からタグを収集
pub fn collect_all_tags(dir: &Path) -> HashSet<String> {
    let mut all_tags = HashSet::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if is_supported_format(&path) {
                for tag in load_tags(&path) {
                    all_tags.insert(tag);
                }
            }
        }
    }
    all_tags
}

// --- XMP helper functions ---

fn build_xmp_packet(tags: &[String]) -> String {
    let mut li = String::new();
    for t in tags {
        li.push_str(&format!("      <rdf:li>{}</rdf:li>\n", xml_escape(t)));
    }

    format!(r#"<?xpacket begin='﻿' id='W5M0MpCehiHzreSzNTczkc9d'?>
<x:xmpmeta xmlns:x='adobe:ns:meta/'>
  <rdf:RDF xmlns:rdf='http://www.w3.org/1999/02/22-rdf-syntax-ns#'>
    <rdf:Description xmlns:dc='http://purl.org/dc/elements/1.1/'>
      <dc:subject>
        <rdf:Bag>
{li}        </rdf:Bag>
      </dc:subject>
    </rdf:Description>
  </rdf:RDF>
</x:xmpmeta>
<?xpacket end='w'?>"#)
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
     .replace('<', "&lt;")
     .replace('>', "&gt;")
     .replace('"', "&quot;")
     .replace('\'', "&apos;")
}

fn parse_xmp_subjects(xmp: &str) -> Vec<String> {
    let mut reader = Reader::from_str(xmp);
    reader.trim_text(true);
    let mut buf = Vec::new();
    let mut tags = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                if e.name().as_ref().ends_with(b"li") {
                    if let Ok(text) = reader.read_text(e.name()) {
                        let t = text.trim().to_string();
                        if !t.is_empty() {
                            tags.push(t);
                        }
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }

    tags
}

fn is_jpeg(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|s| matches!(s.to_lowercase().as_str(), "jpg" | "jpeg"))
        .unwrap_or(false)
}

fn is_png(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_lowercase() == "png")
        .unwrap_or(false)
}

fn extract_xmp_from_bytes(data: &[u8]) -> Option<String> {
    // Try WebP XMP first
    if data.len() > 12 && &data[..4] == b"RIFF" && &data[8..12] == b"WEBP" {
        if let Some(s) = extract_xmp_from_webp_bytes(data) {
            return Some(s);
        }
    }

    // Try JPEG APP1 parsing
    if data.len() > 2 && data[0] == 0xFF && data[1] == 0xD8 {
        let mut i = 2;
        while i + 4 < data.len() {
            if data[i] != 0xFF {
                break;
            }
            let marker = data[i + 1];
            let len = ((data[i + 2] as usize) << 8) | (data[i + 3] as usize);
            if marker == 0xE1 { // APP1
                let start = i + 4;
                if start + len - 2 <= data.len() {
                    let payload = &data[start..start + len - 2];
                    let xmp_sig = b"http://ns.adobe.com/xap/1.0/\0";
                    if payload.starts_with(xmp_sig) {
                        if let Ok(s) = std::str::from_utf8(&payload[xmp_sig.len()..]) {
                            return Some(s.to_string());
                        }
                    }
                }
            }
            i += 2 + len;
        }
    }

    // Try to find raw <?xpacket in file (fallback)
    if let Ok(s) = std::str::from_utf8(data) {
        if let Some(start) = s.find("<?xpacket") {
            if let Some(end) = s.find("?>") {
                // Try to find closing xpacket end marker
                if let Some(end_marker) = s.find("<?xpacket end=") {
                    if let Some(end_pos) = s[end_marker..].find("?>") {
                        let end_index = end_marker + end_pos + 2;
                        return Some(s[start..end_index].to_string());
                    }
                }
                // fallback: return from <?xpacket to first ?>
                return Some(s[start..end + 2].to_string());
            }
        }
    }

    // PNG XMP iTXt search: look for "<x:xmpmeta"
    if let Ok(s) = std::str::from_utf8(data) {
        if let Some(pos) = s.find("<x:xmpmeta") {
            if let Some(end) = s[pos..].find("?>") {
                // crude: return from start to end+2
                let endpos = pos + end + 2;
                return Some(s[pos..endpos].to_string());
            }
        }
    }

    None
}

fn extract_xmp_from_webp_bytes(data: &[u8]) -> Option<String> {
    // RIFF 'RIFF' (4) + size (4 LE) + 'WEBP' (4) + chunks...
    if data.len() < 12 || &data[..4] != b"RIFF" || &data[8..12] != b"WEBP" {
        return None;
    }

    let mut pos = 12usize;
    while pos + 8 <= data.len() {
        let id = &data[pos..pos+4];
        let size = u32::from_le_bytes(data[pos+4..pos+8].try_into().unwrap()) as usize;
        let start = pos + 8;
        let end = start + size;
        if end > data.len() { break; }
        if id == b"XMP " {
            if let Ok(s) = std::str::from_utf8(&data[start..end]) {
                return Some(s.to_string());
            }
        }
        pos = end + (size & 1) as usize; // pad to even
    }

    None
}

fn write_xmp_to_webp_bytes(data: &[u8], xmp: &[u8]) -> std::io::Result<Vec<u8>> {
    if data.len() < 12 || &data[..4] != b"RIFF" || &data[8..12] != b"WEBP" {
        return Ok(data.to_vec());
    }

    // parse chunks into vector
    let mut chunks: Vec<([u8;4], Vec<u8>)> = Vec::new();
    let mut pos = 12usize;
    while pos + 8 <= data.len() {
        let id: [u8;4] = data[pos..pos+4].try_into().unwrap();
        let size = u32::from_le_bytes(data[pos+4..pos+8].try_into().unwrap()) as usize;
        let start = pos + 8;
        let end = start + size;
        if end > data.len() { break; }
        let mut buf = Vec::new();
        buf.extend_from_slice(&data[start..end]);
        chunks.push((id, buf));
        pos = end + (size & 1) as usize;
    }

    // replace or append XMP chunk
    let mut replaced = false;
    for (id, ref mut buf) in &mut chunks {
        if id == b"XMP " {
            *buf = xmp.to_vec();
            replaced = true;
            break;
        }
    }
    if !replaced {
        chunks.push((*b"XMP ", xmp.to_vec()));
    }

    // rebuild RIFF
    let mut out: Vec<u8> = Vec::new();
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&0u32.to_le_bytes()); // placeholder for size
    out.extend_from_slice(b"WEBP");

    for (id, buf) in &chunks {
        out.extend_from_slice(id);
        out.extend_from_slice(&(buf.len() as u32).to_le_bytes());
        out.extend_from_slice(buf);
        if (buf.len() & 1) == 1 {
            out.push(0);
        }
    }

    // set riff size (file size - 8)
    let riff_size = (out.len() - 8) as u32;
    out[4..8].copy_from_slice(&riff_size.to_le_bytes());

    Ok(out)
}

fn write_xmp_to_jpeg_bytes(data: &[u8], xmp: &[u8]) -> Vec<u8> {
    // Build APP1 payload: "http://ns.adobe.com/xap/1.0/\0" + xmp
    let sig = b"http://ns.adobe.com/xap/1.0/\0";
    let mut payload = Vec::new();
    payload.extend_from_slice(sig);
    payload.extend_from_slice(xmp);

    // Construct APP1 segment: 0xFFE1, length (2 bytes), payload
    let seg_len = (payload.len() + 2) as u16; // length includes the two length bytes
    let mut app1 = Vec::new();
    app1.push(0xFF);
    app1.push(0xE1);
    app1.push((seg_len >> 8) as u8);
    app1.push((seg_len & 0xFF) as u8);
    app1.extend_from_slice(&payload);

    // If existing APP1 xmp exists, replace it; otherwise insert after SOI
    if data.len() > 2 && data[0] == 0xFF && data[1] == 0xD8 {
        let mut i = 2;
        while i + 4 < data.len() {
            if data[i] != 0xFF { break; }
            let marker = data[i+1];
            let len = ((data[i+2] as usize) << 8) | (data[i+3] as usize);
            if marker == 0xE1 {
                let start = i+4;
                if start + len - 2 <= data.len() {
                    let payload = &data[start..start+len-2];
                    let xmp_sig = b"http://ns.adobe.com/xap/1.0/\0";
                    if payload.starts_with(xmp_sig) {
                        // replace this segment
                        let mut out = Vec::new();
                        out.extend_from_slice(&data[..i]);
                        out.extend_from_slice(&app1);
                        out.extend_from_slice(&data[i+2+len..]);
                        return out;
                    }
                }
            }
            i += 2 + len;
        }
        // insert after SOI
        let mut out = Vec::new();
        out.extend_from_slice(&data[..2]);
        out.extend_from_slice(&app1);
        out.extend_from_slice(&data[2..]);
        return out;
    }

    // fallback: return original
    data.to_vec()
}

fn write_xmp_to_png_bytes(data: &[u8], xmp: &[u8]) -> std::io::Result<Vec<u8>> {
    // Parse PNG chunks and insert/replace iTXt with keyword "XML:com.adobe.xmp"
    // PNG signature is 8 bytes
    if data.len() < 8 || &data[..8] != b"\x89PNG\r\n\x1a\n" {
        return Ok(data.to_vec());
    }

    let mut pos = 8usize;
    let mut out = Vec::new();
    out.extend_from_slice(&data[..8]);

    let mut replaced = false;
    while pos + 8 <= data.len() {
        let len = u32::from_be_bytes(data[pos..pos+4].try_into().unwrap()) as usize;
        let ctype = &data[pos+4..pos+8];
        let cdata_start = pos + 8;
        let cdata_end = cdata_start + len;
        if cdata_end + 4 > data.len() { break; }

        let cdata = &data[cdata_start..cdata_end];
        let crc = &data[cdata_end..cdata_end+4];

        if ctype == b"iTXt" {
            // check keyword
            if let Some(0) = cdata.iter().position(|b| *b == 0) {
                // keyword ends at first null
                let mut idx = 0usize;
                while idx < cdata.len() && cdata[idx] != 0 { idx +=1; }
                let keyword = &cdata[..idx];
                if keyword == b"XML:com.adobe.xmp" || keyword == b"xml:com.adobe.xmp" {
                    // replace this chunk with new iTXt
                    let new_chunk = build_png_itxt_chunk(b"XML:com.adobe.xmp", xmp)?;
                    out.extend_from_slice(&new_chunk);
                    replaced = true;
                    pos = cdata_end + 4;
                    continue;
                }
            }
        }

        // copy original chunk
        out.extend_from_slice(&data[pos..cdata_end+4]);
        // stop at IEND
        if ctype == b"IEND" {
            break;
        }
        pos = cdata_end + 4;
    }

    if !replaced {
        // insert new iTXt before IEND
        // find IEND position by scanning again
        let mut pos = 8usize;
        while pos + 8 <= data.len() {
            let len = u32::from_be_bytes(data[pos..pos+4].try_into().unwrap()) as usize;
            let ctype = &data[pos+4..pos+8];
            let cdata_start = pos + 8;
            let cdata_end = cdata_start + len;
            if cdata_end + 4 > data.len() { break; }
            if ctype == b"IEND" {
                // insert before this
                let mut out2 = Vec::new();
                out2.extend_from_slice(&data[..pos]);
                let new_chunk = build_png_itxt_chunk(b"XML:com.adobe.xmp", xmp)?;
                out2.extend_from_slice(&new_chunk);
                out2.extend_from_slice(&data[pos..]);
                return Ok(out2);
            }
            pos = cdata_end + 4;
        }
        // if no IEND found, just append
        let mut out2 = out;
        let new_chunk = build_png_itxt_chunk(b"XML:com.adobe.xmp", xmp)?;
        out2.extend_from_slice(&new_chunk);
        return Ok(out2);
    }

    Ok(out)
}

fn build_png_itxt_chunk(keyword: &[u8], text: &[u8]) -> std::io::Result<Vec<u8>> {
    // iTXt: keyword\0 compression_flag(1) compression_method(1) language_tag\0 translated_keyword\0 text
    let mut data = Vec::new();
    data.extend_from_slice(keyword);
    data.push(0);
    data.push(0); // compression flag = 0 (uncompressed)
    data.push(0); // compression method
    data.push(0); // empty language tag null-terminated
    data.push(0); // empty translated keyword null-terminated
    data.extend_from_slice(text);

    let mut chunk = Vec::new();
    let len = (data.len() as u32).to_be_bytes();
    chunk.extend_from_slice(&len);
    chunk.extend_from_slice(b"iTXt");
    chunk.extend_from_slice(&data);

    let mut hasher = Crc32Hasher::new();
    hasher.update(b"iTXt");
    hasher.update(&data);
    let crc = hasher.finalize().to_be_bytes();
    chunk.extend_from_slice(&crc);
    Ok(chunk)
}

/// メタデータ埋め込みに対応しているフォーマットか判定
pub fn is_supported_format(path: &Path) -> bool {
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        matches!(
            ext.to_lowercase().as_str(),
            "png" | "jpg" | "jpeg" | "webp"
        )
    } else {
        false
    }
}

/// ファイルが画像かどうかを判定 (表示用)
pub fn is_image_file(path: &Path) -> bool {
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        matches!(
            ext.to_lowercase().as_str(),
            "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp"
        )
    } else {
        false
    }
}

/// 特定のタグを持つ画像を検索
pub fn find_images_with_tag(dir: &Path, tag: &str) -> Vec<PathBuf> {
    let mut result = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if is_supported_format(&path) {
                let tags = load_tags(&path);
                if tags.iter().any(|t| t == tag) {
                    result.push(path);
                }
            }
        }
    }
    result.sort();
    result
}
