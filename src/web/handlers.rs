use std::collections::{BTreeMap, HashSet};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use percent_encoding::percent_decode_str;
use tiny_http::{Header, Request, Response};

use super::router::json_response;
use super::server_state::ServerState;
use crate::filter;
use crate::metadata::{self, is_image_file, is_metadata_supported};

pub fn tree(query: &str) -> Response<io::Cursor<Vec<u8>>> {
    let params = parse_query(query);
    let dir = match params.get("path") {
        Some(p) => PathBuf::from(p),
        None => match home_dir() {
            Some(d) => d,
            None => return error_json(400, "missing 'path' parameter"),
        },
    };
    if !dir.is_dir() {
        return error_json(400, "path is not a directory");
    }

    let mut dirs: Vec<String> = Vec::new();
    let mut files: Vec<String> = Vec::new();

    let entries = match std::fs::read_dir(&dir) {
        Ok(e) => e,
        Err(e) => return error_json(500, &format!("read_dir failed: {}", e)),
    };
    for entry in entries.flatten() {
        let p = entry.path();
        let Some(name) = p.file_name().and_then(|n| n.to_str()).map(str::to_owned) else {
            continue;
        };
        if p.is_dir() {
            dirs.push(name);
        } else if is_image_file(&p) {
            files.push(name);
        }
    }
    dirs.sort_by_key(|a| a.to_lowercase());
    files.sort_by_key(|a| a.to_lowercase());

    let parent = dir.parent().map(|p| p.display().to_string());
    let body = format!(
        "{{\"path\":{},\"parent\":{},\"dirs\":{},\"files\":{}}}",
        json_string(&dir.display().to_string()),
        json_optional_string(parent.as_deref()),
        json_array(&dirs),
        json_array(&files),
    );
    json_response(200, body)
}

pub fn image(request: Request, query: &str) -> io::Result<()> {
    let path = match resolve_image_path(query) {
        Ok(p) => p,
        Err(msg) => return request.respond(error_json(400, msg)),
    };
    serve_file(request, &path)
}

pub fn thumb(request: Request, query: &str) -> io::Result<()> {
    let path = match resolve_image_path(query) {
        Ok(p) => p,
        Err(msg) => return request.respond(error_json(400, msg)),
    };
    let params = parse_query(query);
    let size: u32 = params
        .get("size")
        .and_then(|s| s.parse().ok())
        .unwrap_or(128)
        .clamp(16, 512);

    let img = match image::open(&path) {
        Ok(i) => i.thumbnail(size, size),
        Err(e) => return request.respond(error_json(500, &format!("decode failed: {}", e))),
    };
    let mut buf: Vec<u8> = Vec::new();
    if let Err(e) = img.write_to(&mut io::Cursor::new(&mut buf), image::ImageFormat::Jpeg) {
        return request.respond(error_json(500, &format!("encode failed: {}", e)));
    }
    let response = Response::from_data(buf).with_header(content_type_header("image/jpeg"));
    request.respond(response)
}

pub fn get_tags(query: &str) -> Response<io::Cursor<Vec<u8>>> {
    let path = match resolve_image_path(query) {
        Ok(p) => p,
        Err(msg) => return error_json(400, msg),
    };
    if !is_metadata_supported(&path) {
        return error_json(400, "format does not support tags");
    }
    let tags = metadata::load_tags(&path);
    let body = format!("{{\"tags\":{}}}", json_array(&tags));
    json_response(200, body)
}

pub fn put_tags(request: &mut Request, query: &str) -> Response<io::Cursor<Vec<u8>>> {
    let path = match resolve_image_path(query) {
        Ok(p) => p,
        Err(msg) => return error_json(400, msg),
    };
    let mut body = String::new();
    if let Err(e) = request.as_reader().read_to_string(&mut body) {
        return error_json(400, &format!("read body failed: {}", e));
    }
    let tags = match parse_tags_body(&body) {
        Ok(t) => t,
        Err(msg) => return error_json(400, msg),
    };
    if let Err(e) = metadata::save_tags(&path, &tags) {
        return error_json(500, &format!("save failed: {}", e));
    }
    json_response(200, "{\"ok\":true}".to_string())
}

pub fn hotkeys(state: &Arc<ServerState>) -> Response<io::Cursor<Vec<u8>>> {
    let cfg = state.config.lock().expect("config mutex poisoned");
    let pairs: BTreeMap<String, String> = cfg.hotkey_tags.clone().into_iter().collect();
    let body = format!("{{\"hotkeys\":{}}}", json_object(&pairs));
    json_response(200, body)
}

pub fn filter(query: &str) -> Response<io::Cursor<Vec<u8>>> {
    let params = parse_query(query);
    let Some(dir_str) = params.get("path") else {
        return error_json(400, "missing 'path' parameter");
    };
    let dir = PathBuf::from(dir_str);
    if !dir.is_dir() {
        return error_json(400, "path is not a directory");
    }

    let required: HashSet<String> = params
        .get("tags")
        .map(|s| {
            s.split(',')
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty())
                .collect()
        })
        .unwrap_or_default();
    let free_word = params.get("q").cloned().unwrap_or_default();

    let entries = match std::fs::read_dir(&dir) {
        Ok(e) => e,
        Err(e) => return error_json(500, &format!("read_dir failed: {}", e)),
    };
    let mut matches: Vec<String> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| is_metadata_supported(p))
        .filter(|p| filter::matches(&metadata::load_tags(p), &required, &free_word))
        .filter_map(|p| p.file_name().and_then(|n| n.to_str()).map(str::to_owned))
        .collect();
    matches.sort_by_key(|a| a.to_lowercase());

    let body = format!("{{\"matches\":{}}}", json_array(&matches));
    json_response(200, body)
}

// -----------------------------------------------------------------------------
// Helpers
// -----------------------------------------------------------------------------

fn serve_file(request: Request, path: &Path) -> io::Result<()> {
    if !path.is_file() {
        return request.respond(error_json(404, "file not found"));
    }
    let data = match std::fs::read(path) {
        Ok(d) => d,
        Err(e) => return request.respond(error_json(500, &format!("read failed: {}", e))),
    };
    let mime = guess_mime(path);
    let response = Response::from_data(data).with_header(content_type_header(mime));
    request.respond(response)
}

fn resolve_image_path(query: &str) -> Result<PathBuf, &'static str> {
    let params = parse_query(query);
    let raw = params.get("path").ok_or("missing 'path' parameter")?;
    let path = PathBuf::from(raw);
    if !path.is_absolute() {
        return Err("'path' must be absolute");
    }
    Ok(path)
}

fn parse_query(query: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for pair in query.split('&').filter(|p| !p.is_empty()) {
        let (key, value) = match pair.find('=') {
            Some(idx) => (&pair[..idx], &pair[idx + 1..]),
            None => (pair, ""),
        };
        let k = decode(key);
        let v = decode(value);
        out.insert(k, v);
    }
    out
}

fn decode(s: &str) -> String {
    // application/x-www-form-urlencoded uses '+' for space.
    let replaced = s.replace('+', " ");
    percent_decode_str(&replaced).decode_utf8_lossy().into_owned()
}

fn parse_tags_body(body: &str) -> Result<Vec<String>, &'static str> {
    let value: serde_json::Value = serde_json::from_str(body).map_err(|_| "invalid JSON body")?;
    let arr = value
        .get("tags")
        .and_then(|v| v.as_array())
        .ok_or("body must be {\"tags\": [...]}")?;
    let mut out = Vec::with_capacity(arr.len());
    for item in arr {
        let s = item.as_str().ok_or("tags must be strings")?;
        out.push(s.to_string());
    }
    Ok(out)
}

fn home_dir() -> Option<PathBuf> {
    dirs::home_dir()
}

fn content_type_header(value: &str) -> Header {
    Header::from_bytes(b"Content-Type".as_slice(), value.as_bytes())
        .expect("valid Content-Type header")
}

fn guess_mime(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
        .as_deref()
    {
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("bmp") => "image/bmp",
        _ => "application/octet-stream",
    }
}

fn error_json(status: u16, message: &str) -> Response<io::Cursor<Vec<u8>>> {
    let body = format!("{{\"error\":{}}}", json_string(message));
    json_response(status, body)
}

// -----------------------------------------------------------------------------
// Tiny manual JSON serialization. We avoid pulling serde_json::to_string for the
// few simple shapes here because the keys are fixed and known to be safe.
// -----------------------------------------------------------------------------

fn json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn json_optional_string(value: Option<&str>) -> String {
    match value {
        Some(s) => json_string(s),
        None => "null".to_string(),
    }
}

fn json_array(items: &[String]) -> String {
    let parts: Vec<String> = items.iter().map(|s| json_string(s)).collect();
    format!("[{}]", parts.join(","))
}

fn json_object(map: &BTreeMap<String, String>) -> String {
    let mut parts: Vec<String> = Vec::with_capacity(map.len());
    for (k, v) in map {
        parts.push(format!("{}:{}", json_string(k), json_string(v)));
    }
    format!("{{{}}}", parts.join(","))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_string_escapes_quotes_and_backslashes() {
        assert_eq!(json_string("a\"b"), "\"a\\\"b\"");
        assert_eq!(json_string("a\\b"), "\"a\\\\b\"");
    }

    #[test]
    fn parse_query_decodes_percent_and_plus() {
        let q = "path=%2Ftmp%2Fa+b&tags=cat%2Cdog";
        let m = parse_query(q);
        assert_eq!(m.get("path").map(String::as_str), Some("/tmp/a b"));
        assert_eq!(m.get("tags").map(String::as_str), Some("cat,dog"));
    }

    #[test]
    fn parse_tags_body_extracts_array() {
        let body = r#"{"tags":["a","b"]}"#;
        let parsed = parse_tags_body(body).unwrap();
        assert_eq!(parsed, vec!["a".to_string(), "b".to_string()]);
    }

    #[test]
    fn parse_tags_body_rejects_non_array() {
        assert!(parse_tags_body(r#"{"tags":"a"}"#).is_err());
        assert!(parse_tags_body("not-json").is_err());
    }
}
