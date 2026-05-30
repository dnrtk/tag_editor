use std::fs::{self, File};
use std::io::BufReader;
use std::path::{Path, PathBuf};

mod cache;
mod error;
mod format;
mod jpeg;
mod png;
mod webp;
mod xmp;

pub use error::{MetadataError, Result};
pub use format::{is_image_file, is_metadata_supported, ImageFormat};

/// Persists the in-memory tag cache to disk. Call after a scan completes and on
/// app exit so repeat scans of an unchanged library skip re-reading every file.
pub fn flush_cache() {
    cache::flush();
}

/// Loads an image's tags, serving them from the persistent mtime-keyed cache when
/// the file is unchanged since it was last read.
pub fn load_tags(path: &Path) -> Vec<String> {
    cache::get_or_load(path, load_tags_uncached)
}

/// Reads tags straight from the file, bypassing the cache. Streams only the
/// metadata segments — the image body is skipped via seek, so a multi-megabyte
/// photo costs a few small reads instead of a full file read.
fn load_tags_uncached(path: &Path) -> Vec<String> {
    let Some(format) = ImageFormat::from_path(path) else {
        return Vec::new();
    };
    let Ok(file) = File::open(path) else {
        return Vec::new();
    };
    let mut reader = BufReader::new(file);
    let packet = match format {
        ImageFormat::Jpeg => jpeg::read_xmp_streaming(&mut reader),
        ImageFormat::Png => png::read_xmp_streaming(&mut reader),
        ImageFormat::Webp => webp::read_xmp_streaming(&mut reader),
    };
    packet
        .map(|xmp| xmp::parse_subjects(&xmp))
        .unwrap_or_default()
}

pub fn save_tags(path: &Path, tags: &[String]) -> Result<()> {
    let format = ImageFormat::from_path(path).ok_or(MetadataError::UnsupportedFormat)?;
    let xmp = xmp::build_packet(tags);
    let original = fs::read(path)?;
    let new_data = embed_xmp(format, &original, xmp.as_bytes())?;
    fs::write(path, new_data)?;
    // Drop the stale cached entry; the next read re-streams the just-written tags.
    cache::invalidate(path);
    Ok(())
}

pub fn add_tag(tags: &mut Vec<String>, tag: &str) {
    let trimmed = tag.trim();
    if !trimmed.is_empty() && !tags.iter().any(|t| t == trimmed) {
        tags.push(trimmed.to_string());
    }
}

pub fn remove_tag(tags: &mut Vec<String>, tag: &str) {
    tags.retain(|t| t != tag);
}

pub fn toggle_tag(tags: &mut Vec<String>, tag: &str) -> bool {
    let trimmed = tag.trim();
    if trimmed.is_empty() {
        return false;
    }
    if let Some(pos) = tags.iter().position(|t| t == trimmed) {
        tags.remove(pos);
        false
    } else {
        tags.push(trimmed.to_string());
        true
    }
}

#[allow(dead_code)]
pub fn find_images_with_tag(dir: &Path, tag: &str) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut result: Vec<PathBuf> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| is_metadata_supported(p))
        .filter(|p| load_tags(p).iter().any(|t| t == tag))
        .collect();
    result.sort();
    result
}

fn embed_xmp(format: ImageFormat, data: &[u8], xmp: &[u8]) -> Result<Vec<u8>> {
    match format {
        ImageFormat::Jpeg => jpeg::write_xmp(data, xmp),
        ImageFormat::Png => png::write_xmp(data, xmp),
        ImageFormat::Webp => webp::write_xmp(data, xmp),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Rgb};

    fn write_test_image(path: &Path) {
        let img = ImageBuffer::<Rgb<u8>, _>::from_fn(8, 8, |x, y| {
            Rgb([(x * 32) as u8, (y * 32) as u8, 128])
        });
        img.save(path).expect("save test image");
    }

    #[test]
    fn jpeg_tag_round_trip() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("test.jpg");
        write_test_image(&path);

        assert!(load_tags(&path).is_empty());

        let tags = vec!["alpha".to_string(), "beta".to_string()];
        save_tags(&path, &tags).expect("save jpeg tags");
        assert_eq!(load_tags(&path), tags);

        let tags2 = vec!["gamma".to_string()];
        save_tags(&path, &tags2).expect("save jpeg tags second time");
        assert_eq!(load_tags(&path), tags2);
    }

    #[test]
    fn png_tag_round_trip() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("test.png");
        write_test_image(&path);

        assert!(load_tags(&path).is_empty());

        let tags = vec!["one".to_string(), "two".to_string(), "three".to_string()];
        save_tags(&path, &tags).expect("save png tags");
        assert_eq!(load_tags(&path), tags);

        let tags2 = vec!["four".to_string()];
        save_tags(&path, &tags2).expect("save png tags second time");
        assert_eq!(load_tags(&path), tags2);
    }

    #[test]
    fn special_characters_round_trip_jpeg() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("test.jpg");
        write_test_image(&path);

        let tags = vec!["a&b".to_string(), "<weird>".to_string(), "猫".to_string()];
        save_tags(&path, &tags).expect("save");
        assert_eq!(load_tags(&path), tags);
    }

    #[test]
    fn special_characters_round_trip_png() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("test.png");
        write_test_image(&path);

        let tags = vec!["a&b".to_string(), "<weird>".to_string(), "犬".to_string()];
        save_tags(&path, &tags).expect("save");
        assert_eq!(load_tags(&path), tags);
    }

    #[test]
    fn add_tag_dedupes_and_trims() {
        let mut tags = vec!["cat".to_string()];
        add_tag(&mut tags, "  cat  ");
        add_tag(&mut tags, "");
        add_tag(&mut tags, "dog");
        assert_eq!(tags, vec!["cat".to_string(), "dog".to_string()]);
    }

    #[test]
    fn toggle_adds_then_removes() {
        let mut tags = Vec::new();
        assert!(toggle_tag(&mut tags, "x"));
        assert_eq!(tags, vec!["x".to_string()]);
        assert!(!toggle_tag(&mut tags, "x"));
        assert!(tags.is_empty());
    }
}
