use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Clone, Serialize, Deserialize)]
pub struct Config {
    /// ホットキー（キー文字列）に対応するタグ
    pub hotkey_tags: HashMap<String, String>,
    /// オートセーブの有効/無効
    pub auto_save: bool,
    /// スライドショーの切り替え間隔（秒）
    pub slideshow_interval: f32,
    /// スライドショーをループするか
    pub slideshow_loop: bool,
    /// 左サイドバーの表示
    pub show_left_sidebar: bool,
    /// 右サイドバーの表示
    pub show_right_sidebar: bool,
    
    // ウィンドウサイズ (width, height)
    pub left_window_size: Option<[f32; 2]>,
    pub right_window_size: Option<[f32; 2]>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            hotkey_tags: HashMap::new(),
            auto_save: false,
            slideshow_interval: 3.0,
            slideshow_loop: true,
            show_left_sidebar: false,
            show_right_sidebar: false,
            left_window_size: None,
            right_window_size: None,
        }
    }
}

impl Config {
    fn config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("tag_editor")
            .join("config.json")
    }

    pub fn load() -> Self {
        let mut config = Self::default();
        
        // 1. 通常のconfig.jsonを読み込み
        let path = Self::config_path();
        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(loaded) = serde_json::from_str::<Config>(&content) {
                    config = loaded;
                }
            }
        }
        
        // 2. exeと同じディレクトリのsettings.jsonを読み込み（あればマージ）
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                let settings_path = exe_dir.join("settings.json");
                if settings_path.exists() {
                    if let Ok(content) = fs::read_to_string(&settings_path) {
                        if let Ok(settings) = serde_json::from_str::<serde_json::Value>(&content) {
                            if let Some(hotkeys) = settings.get("hotkey_tags").and_then(|v| v.as_object()) {
                                config.hotkey_tags.clear(); // config.jsonの設定をsettings.jsonで完全に上書きする場合
                                for (k, v) in hotkeys {
                                    if let Some(tag) = v.as_str() {
                                        config.hotkey_tags.insert(k.clone(), tag.to_string());
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        
        config
    }

    pub fn save(&self) {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        // settings.jsonにある項目はconfig.jsonには保存しないのが理想だが、
        // 簡易実装として全部config.jsonに保存してしまう（読み込み時にsettings.jsonが優先されるので動作はする）
        if let Ok(content) = serde_json::to_string_pretty(self) {
            let _ = fs::write(&path, content);
        }
    }
}
