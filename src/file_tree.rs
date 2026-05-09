use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::metadata::is_image_file;

#[derive(Debug, Clone)]
pub struct FileNode {
    pub path: PathBuf,
    pub name: String,
    pub is_dir: bool,
    pub children: Vec<FileNode>,
    /// True once `load_children` has populated `children` for this directory.
    loaded: bool,
}

impl FileNode {
    fn new(path: PathBuf) -> Self {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        let is_dir = path.is_dir();
        Self {
            path,
            name,
            is_dir,
            children: Vec::new(),
            loaded: false,
        }
    }

    fn load_children(&mut self) {
        if !self.is_dir || self.loaded {
            return;
        }
        self.loaded = true;
        self.children.clear();

        let Ok(entries) = std::fs::read_dir(&self.path) else {
            return;
        };

        let mut dirs = Vec::new();
        let mut files = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                dirs.push(FileNode::new(path));
            } else if is_image_file(&path) {
                files.push(FileNode::new(path));
            }
        }
        let by_name = |a: &FileNode, b: &FileNode| a.name.to_lowercase().cmp(&b.name.to_lowercase());
        dirs.sort_by(by_name);
        files.sort_by(by_name);

        self.children.reserve(dirs.len() + files.len());
        self.children.extend(dirs);
        self.children.extend(files);
    }
}

#[derive(Default)]
pub struct FileTree {
    pub root: Option<FileNode>,
    expanded: HashSet<PathBuf>,
}

impl FileTree {
    pub fn set_root(&mut self, path: &Path) {
        let root_path = if path.is_dir() {
            path.to_path_buf()
        } else if let Some(parent) = path.parent() {
            parent.to_path_buf()
        } else {
            return;
        };

        let mut root = FileNode::new(root_path.clone());
        root.load_children();
        self.expanded.clear();
        self.expanded.insert(root_path);
        self.root = Some(root);
    }

    pub fn is_expanded(&self, path: &Path) -> bool {
        self.expanded.contains(path)
    }

    pub fn toggle_expanded(&mut self, path: &Path) {
        if !self.expanded.remove(path) {
            self.expanded.insert(path.to_path_buf());
            if let Some(root) = self.root.as_mut() {
                load_descendant(root, path);
            }
        }
    }
}

/// Walks the tree to `target` and triggers a lazy children load on it.
fn load_descendant(node: &mut FileNode, target: &Path) {
    if node.path == target {
        node.load_children();
        return;
    }
    for child in &mut node.children {
        if target.starts_with(&child.path) {
            load_descendant(child, target);
        }
    }
}
