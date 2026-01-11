use crate::tag_manager::is_image_file;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct FileNode {
    pub path: PathBuf,
    pub name: String,
    pub is_dir: bool,
    pub children: Vec<FileNode>,
}

impl FileNode {
    pub fn new(path: PathBuf) -> Self {
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
        }
    }

    /// ディレクトリの子要素を読み込む
    pub fn load_children(&mut self) {
        if !self.is_dir {
            return;
        }

        self.children.clear();
        if let Ok(entries) = std::fs::read_dir(&self.path) {
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

            // ディレクトリを先に、その後ファイルをソートして追加
            dirs.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
            files.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

            self.children.extend(dirs);
            self.children.extend(files);
        }
    }
}

pub struct FileTree {
    pub root: Option<FileNode>,
    pub expanded: HashSet<PathBuf>,
}

impl Default for FileTree {
    fn default() -> Self {
        Self {
            root: None,
            expanded: HashSet::new(),
        }
    }
}

impl FileTree {
    pub fn set_root(&mut self, path: &Path) {
        if path.is_dir() {
            let mut root = FileNode::new(path.to_path_buf());
            root.load_children();
            self.expanded.insert(path.to_path_buf());
            self.root = Some(root);
        } else if let Some(parent) = path.parent() {
            let mut root = FileNode::new(parent.to_path_buf());
            root.load_children();
            self.expanded.insert(parent.to_path_buf());
            self.root = Some(root);
        }
    }

    pub fn toggle_expanded(&mut self, path: &Path) {
        if self.expanded.contains(path) {
            self.expanded.remove(path);
        } else {
            self.expanded.insert(path.to_path_buf());
            // 子要素を読み込む
            self.load_children_for_path(path);
        }
    }

    pub fn is_expanded(&self, path: &Path) -> bool {
        self.expanded.contains(path)
    }

    fn load_children_for_path(&mut self, target: &Path) {
        if let Some(ref mut root) = self.root {
            Self::load_children_recursive(root, target);
        }
    }

    fn load_children_recursive(node: &mut FileNode, target: &Path) {
        if node.path == target {
            node.load_children();
            return;
        }
        for child in &mut node.children {
            if target.starts_with(&child.path) {
                Self::load_children_recursive(child, target);
            }
        }
    }
}
