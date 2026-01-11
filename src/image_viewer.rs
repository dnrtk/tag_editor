use std::path::{Path, PathBuf};

use crate::tag_manager::is_image_file;

pub struct ImageViewer {
    /// 現在表示中の画像パス
    pub current_image: Option<PathBuf>,
    /// 現在のディレクトリ内の画像リスト
    pub images_in_dir: Vec<PathBuf>,
    /// 現在の画像のインデックス
    pub current_index: usize,
    /// 画像のテクスチャハンドル（egui用）
    texture_uri: Option<String>,
}

impl Default for ImageViewer {
    fn default() -> Self {
        Self {
            current_image: None,
            images_in_dir: Vec::new(),
            current_index: 0,
            texture_uri: None,
        }
    }
}

impl ImageViewer {
    /// パスをfile:// URIに変換（Windows対応）
    fn path_to_uri(path: &Path) -> String {
        // 絶対パスに正規化
        let abs_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir()
                .map(|cwd| cwd.join(path))
                .unwrap_or_else(|_| path.to_path_buf())
        };
        
        // canonicalizeでパスを正規化（シンボリックリンク解決など）
        let canonical = abs_path.canonicalize().unwrap_or(abs_path);
        
        // Windowsパスを正規化してスラッシュに変換
        let path_str = canonical.to_string_lossy();
        
        // Windows UNCパス形式の接頭辞を除去 (\\?\)
        let cleaned = if path_str.starts_with(r"\\?\") {
            &path_str[4..]
        } else {
            &path_str
        };
        
        // バックスラッシュをスラッシュに変換
        let normalized = cleaned.replace('\\', "/");
        
        // file:/// で始まる URI を生成（Windowsではスラッシュ3つ必要）
        format!("file:///{}", normalized)
    }

    pub fn close(&mut self) {
        self.current_image = None;
        self.texture_uri = None;
        self.current_index = 0;
        // images_in_dir は保持してもクリアしてもよいが、ここでは保持する（ディレクトリ移動ではないため）
    }

    /// 画像を開く
    pub fn open(&mut self, path: &Path) {
        if !path.exists() || !is_image_file(path) {
            return;
        }

        self.current_image = Some(path.to_path_buf());
        self.texture_uri = Some(Self::path_to_uri(path));

        // 同じディレクトリ内の画像リストを更新
        if let Some(parent) = path.parent() {
            self.load_directory_images(parent);
            // 現在の画像のインデックスを見つける
            self.current_index = self
                .images_in_dir
                .iter()
                .position(|p| p == path)
                .unwrap_or(0);
        }
    }

    /// ディレクトリ内の画像を読み込む
    fn load_directory_images(&mut self, dir: &Path) {
        self.images_in_dir.clear();
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if is_image_file(&path) {
                    self.images_in_dir.push(path);
                }
            }
        }
        self.images_in_dir.sort();
    }

    /// 前の画像に移動
    pub fn prev(&mut self) {
        if self.images_in_dir.is_empty() {
            return;
        }
        if self.current_index > 0 {
            self.current_index -= 1;
        } else {
            self.current_index = self.images_in_dir.len() - 1;
        }
        if let Some(path) = self.images_in_dir.get(self.current_index).cloned() {
            self.current_image = Some(path.clone());
            self.texture_uri = Some(Self::path_to_uri(&path));
        }
    }

    /// 次の画像に移動
    pub fn next(&mut self) {
        if self.images_in_dir.is_empty() {
            return;
        }
        self.current_index = (self.current_index + 1) % self.images_in_dir.len();
        if let Some(path) = self.images_in_dir.get(self.current_index).cloned() {
            self.current_image = Some(path.clone());
            self.texture_uri = Some(Self::path_to_uri(&path));
        }
    }

    /// 指定インデックスの画像に移動
    #[allow(dead_code)]
    pub fn goto(&mut self, index: usize) {
        if index < self.images_in_dir.len() {
            self.current_index = index;
            if let Some(path) = self.images_in_dir.get(self.current_index).cloned() {
                self.current_image = Some(path.clone());
                self.texture_uri = Some(Self::path_to_uri(&path));
            }
        }
    }

    /// テクスチャURIを取得
    pub fn get_texture_uri(&self) -> Option<&str> {
        self.texture_uri.as_deref()
    }

    /// 画像の総数を取得
    pub fn total_images(&self) -> usize {
        self.images_in_dir.len()
    }
}
