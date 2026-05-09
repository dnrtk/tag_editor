use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Clone, Serialize, Deserialize)]
pub struct Config {
    pub hotkey_tags: HashMap<String, String>,
    pub auto_save: bool,
    pub slideshow_interval: f32,
    pub slideshow_loop: bool,
    pub show_left_sidebar: bool,
    pub show_right_sidebar: bool,
    /// True when the left sidebar is docked into the main window.
    /// False renders it as a floating OS viewport.
    #[serde(default = "default_true")]
    pub left_sidebar_docked: bool,
    #[serde(default = "default_true")]
    pub right_sidebar_docked: bool,
    pub left_window_size: Option<[f32; 2]>,
    pub right_window_size: Option<[f32; 2]>,
    pub slideshow_window_size: Option<[f32; 2]>,
    /// Width in points when the sidebar is docked.
    #[serde(default = "default_dock_width")]
    pub left_dock_width: f32,
    #[serde(default = "default_dock_width")]
    pub right_dock_width: f32,
    /// HTTP port for the embedded web server. Bound on 0.0.0.0.
    #[serde(default = "default_web_port")]
    pub web_port: u16,
    /// When false, the embedded web server is not started.
    #[serde(default = "default_true")]
    pub web_enabled: bool,
}

fn default_true() -> bool {
    true
}
fn default_dock_width() -> f32 {
    250.0
}
/// Avoid 80/8080 (commonly squatted) and 5000-range (Windows uses for misc services).
/// 47823 is in the user-port range and not associated with any well-known service.
fn default_web_port() -> u16 {
    47823
}

impl Default for Config {
    fn default() -> Self {
        Self {
            hotkey_tags: HashMap::new(),
            auto_save: false,
            slideshow_interval: 3.0,
            slideshow_loop: true,
            show_left_sidebar: true,
            show_right_sidebar: true,
            left_sidebar_docked: true,
            right_sidebar_docked: true,
            left_window_size: None,
            right_window_size: None,
            slideshow_window_size: None,
            left_dock_width: default_dock_width(),
            right_dock_width: default_dock_width(),
            web_port: default_web_port(),
            web_enabled: true,
        }
    }
}

impl Config {
    pub fn load() -> Self {
        let mut config = Self::load_from(&Self::user_config_path()).unwrap_or_default();
        // Hotkeys defined alongside the executable take precedence over the user config.
        if let Some(path) = Self::exe_settings_path() {
            apply_exe_overrides(&mut config, &path);
        }
        config
    }

    pub fn save(&self) {
        let path = Self::user_config_path();
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(content) = serde_json::to_string_pretty(self) {
            let _ = fs::write(&path, content);
        }
    }

    fn user_config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("tag_editor")
            .join("config.json")
    }

    fn exe_settings_path() -> Option<PathBuf> {
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.join("settings.json")))
    }

    fn load_from(path: &std::path::Path) -> Option<Self> {
        let content = fs::read_to_string(path).ok()?;
        serde_json::from_str(&content).ok()
    }
}

fn apply_exe_overrides(config: &mut Config, path: &std::path::Path) {
    let Ok(content) = fs::read_to_string(path) else {
        return;
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&content) else {
        return;
    };

    if let Some(hotkeys) = value.get("hotkey_tags").and_then(|v| v.as_object()) {
        config.hotkey_tags.clear();
        for (key, val) in hotkeys {
            if let Some(tag) = val.as_str() {
                config.hotkey_tags.insert(key.clone(), tag.to_string());
            }
        }
    }
    if let Some(port) = value.get("web_port").and_then(|v| v.as_u64()) {
        if port <= u16::MAX as u64 {
            config.web_port = port as u16;
        }
    }
    if let Some(enabled) = value.get("web_enabled").and_then(|v| v.as_bool()) {
        config.web_enabled = enabled;
    }
}
