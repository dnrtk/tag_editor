use std::path::{Path, PathBuf};

use crate::metadata::is_image_file;

#[derive(Default)]
pub struct ImageViewer {
    pub current_image: Option<PathBuf>,
    pub images_in_dir: Vec<PathBuf>,
    pub current_index: usize,
    current_dir: Option<PathBuf>,
}

impl ImageViewer {
    pub fn open(&mut self, path: &Path) {
        if !path.exists() || !is_image_file(path) {
            return;
        }

        let parent = path.parent().map(Path::to_path_buf);
        // Reload directory listing only when the directory actually changed.
        if parent != self.current_dir {
            if let Some(dir) = parent.as_ref() {
                self.load_directory_images(dir);
            } else {
                self.images_in_dir.clear();
            }
            self.current_dir = parent;
        }

        self.current_image = Some(path.to_path_buf());
        self.current_index = self
            .images_in_dir
            .iter()
            .position(|p| p == path)
            .unwrap_or(0);
    }

    pub fn close(&mut self) {
        self.current_image = None;
        self.current_index = 0;
    }

    pub fn prev(&mut self) {
        if self.images_in_dir.is_empty() {
            return;
        }
        self.current_index = if self.current_index == 0 {
            self.images_in_dir.len() - 1
        } else {
            self.current_index - 1
        };
        self.current_image = Some(self.images_in_dir[self.current_index].clone());
    }

    pub fn next(&mut self) {
        if self.images_in_dir.is_empty() {
            return;
        }
        self.current_index = (self.current_index + 1) % self.images_in_dir.len();
        self.current_image = Some(self.images_in_dir[self.current_index].clone());
    }

    /// Removes `path` from the directory listing and returns the next image to show
    /// (the entry that took its position, or the previous one if it was the last).
    /// Returns `None` if no images remain.
    pub fn remove(&mut self, path: &Path) -> Option<PathBuf> {
        let pos = self.images_in_dir.iter().position(|p| p == path)?;
        self.images_in_dir.remove(pos);
        if self.images_in_dir.is_empty() {
            self.current_image = None;
            self.current_index = 0;
            return None;
        }
        let next_idx = pos.min(self.images_in_dir.len() - 1);
        self.current_index = next_idx;
        let next_path = self.images_in_dir[next_idx].clone();
        self.current_image = Some(next_path.clone());
        Some(next_path)
    }

    pub fn total_images(&self) -> usize {
        self.images_in_dir.len()
    }

    fn load_directory_images(&mut self, dir: &Path) {
        self.images_in_dir.clear();
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if is_image_file(&path) {
                self.images_in_dir.push(path);
            }
        }
        self.images_in_dir.sort();
    }
}
