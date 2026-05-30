use std::path::{Path, PathBuf};

use rayon::prelude::*;

use crate::metadata::is_metadata_supported;

/// Recursively collects every metadata-capable image under `base`, descending
/// into all subdirectories. Results are sorted for stable ordering. Symlinked
/// directories are followed by `read_dir`/`file_type` like any other directory;
/// pathological symlink cycles are out of scope for this tool's typical use.
///
/// Subdirectories are traversed in parallel across rayon's thread pool, so a wide
/// or deep tree spreads its `read_dir` calls over all cores.
pub fn collect_images_recursive(base: &Path) -> Vec<PathBuf> {
    let mut out = walk(base);
    out.sort();
    out
}

fn walk(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };

    let mut subdirs: Vec<PathBuf> = Vec::new();
    let mut files: Vec<PathBuf> = Vec::new();
    for entry in entries.flatten() {
        // `file_type()` reuses the directory-entry metadata on Windows, avoiding
        // an extra stat syscall per entry that `path.is_dir()` would incur.
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        let path = entry.path();
        if file_type.is_dir() {
            subdirs.push(path);
        } else if is_metadata_supported(&path) {
            files.push(path);
        }
    }

    // Recurse into subdirectories in parallel, then fold their results in.
    let nested: Vec<PathBuf> = subdirs.par_iter().flat_map(|d| walk(d)).collect();
    files.extend(nested);
    files
}

/// Copies each path in `files` into `dest`, preserving its location relative to
/// `base` so subfolder structure is reproduced and same-named files in different
/// subfolders never collide. Files outside `base` fall back to their file name
/// at the destination root.
///
/// Returns the number of files successfully copied and a list of human-readable
/// error messages for the ones that failed (a single failure does not abort the
/// rest of the batch).
pub fn export_preserving_structure(
    base: &Path,
    files: &[PathBuf],
    dest: &Path,
) -> (usize, Vec<String>) {
    let mut copied = 0usize;
    let mut errors = Vec::new();

    for file in files {
        let relative = match file.strip_prefix(base) {
            Ok(rel) => rel.to_path_buf(),
            Err(_) => match file.file_name() {
                Some(name) => PathBuf::from(name),
                None => {
                    errors.push(format!("{}: invalid file name", file.display()));
                    continue;
                }
            },
        };
        let target = dest.join(&relative);

        if let Some(parent) = target.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                errors.push(format!("{}: {}", relative.display(), e));
                continue;
            }
        }
        match std::fs::copy(file, &target) {
            Ok(_) => copied += 1,
            Err(e) => errors.push(format!("{}: {}", relative.display(), e)),
        }
    }

    (copied, errors)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Rgb};
    use std::fs;

    fn write_image(path: &Path) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        let img = ImageBuffer::<Rgb<u8>, _>::from_fn(4, 4, |x, y| Rgb([x as u8, y as u8, 0]));
        img.save(path).expect("save test image");
    }

    #[test]
    fn collects_images_across_subfolders() {
        let tmp = tempfile::TempDir::new().unwrap();
        let base = tmp.path();
        write_image(&base.join("a.png"));
        write_image(&base.join("sub/b.jpg"));
        write_image(&base.join("sub/deep/c.webp"));
        // Non-image and tag-unsupported files must be ignored.
        fs::write(base.join("notes.txt"), b"x").unwrap();
        write_image(&base.join("sub/skip.gif"));

        let found = collect_images_recursive(base);
        assert_eq!(found.len(), 3, "found: {:?}", found);
        assert!(found.iter().any(|p| p.ends_with("a.png")));
        assert!(found.iter().any(|p| p.ends_with("sub/b.jpg") || p.ends_with("sub\\b.jpg")));
        assert!(found
            .iter()
            .any(|p| p.ends_with("sub/deep/c.webp") || p.ends_with("sub\\deep\\c.webp")));
    }

    #[test]
    fn export_preserves_subfolder_structure() {
        let tmp = tempfile::TempDir::new().unwrap();
        let base = tmp.path().join("src");
        let dest = tmp.path().join("out");
        let f1 = base.join("a.png");
        let f2 = base.join("sub/deep/b.png");
        write_image(&f1);
        write_image(&f2);

        let (copied, errors) =
            export_preserving_structure(&base, &[f1.clone(), f2.clone()], &dest);

        assert_eq!(copied, 2);
        assert!(errors.is_empty(), "errors: {:?}", errors);
        assert!(dest.join("a.png").is_file());
        assert!(dest.join("sub/deep/b.png").is_file());
        // Originals remain (copy, not move).
        assert!(f1.is_file());
        assert!(f2.is_file());
    }

    #[test]
    fn same_name_in_different_subfolders_does_not_collide() {
        let tmp = tempfile::TempDir::new().unwrap();
        let base = tmp.path().join("src");
        let dest = tmp.path().join("out");
        let f1 = base.join("x/img.png");
        let f2 = base.join("y/img.png");
        write_image(&f1);
        write_image(&f2);

        let (copied, errors) = export_preserving_structure(&base, &[f1, f2], &dest);

        assert_eq!(copied, 2);
        assert!(errors.is_empty());
        assert!(dest.join("x/img.png").is_file());
        assert!(dest.join("y/img.png").is_file());
    }
}
