use eframe::egui;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::config::Config;
use crate::file_tree::FileTree;
use crate::image_viewer::ImageViewer;
use crate::metadata::{self, is_image_file};
use crate::slideshow::Slideshow;

#[derive(Clone, Copy)]
pub enum NavDirection {
    Prev,
    Next,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SlideshowListView {
    Names,
    Thumbnails,
}

pub struct SlideshowDialog {
    pub open: bool,
    pub free_word: String,
    pub selected_tags: HashSet<String>,
    pub view: SlideshowListView,
    /// Loaded once when the dialog opens to avoid hitting disk on every keystroke.
    pub tag_cache: HashMap<PathBuf, Vec<String>>,
    /// Filtered subset, recomputed when criteria change.
    pub filtered: Vec<PathBuf>,
    pub last_dir: Option<PathBuf>,
}

impl Default for SlideshowDialog {
    fn default() -> Self {
        Self {
            open: false,
            free_word: String::new(),
            selected_tags: HashSet::new(),
            view: SlideshowListView::Names,
            tag_cache: HashMap::new(),
            filtered: Vec::new(),
            last_dir: None,
        }
    }
}

pub struct AppState {
    pub config: Config,
    pub image_viewer: ImageViewer,
    pub file_tree: FileTree,
    pub slideshow: Slideshow,

    pub current_tags: Vec<String>,
    pub tags_modified: bool,

    pub new_tag_input: String,

    pub slideshow_dialog: SlideshowDialog,
    pub slideshow_dir: Option<PathBuf>,

    pub status_message: String,

    /// Sidebar OS-window state at the *previous* frame; used to detect first-frame open.
    pub was_left_sidebar_open: bool,
    pub was_right_sidebar_open: bool,
    pub was_slideshow_dialog_open: bool,

    /// Cached GPU texture for the currently displayed image. Cleared on image switch.
    pub current_texture: Option<egui::TextureHandle>,
    pub current_texture_path: Option<PathBuf>,

    /// Bind URL of the embedded web server, populated by the app on startup. None if
    /// the server failed to start (port in use, etc.) or `web_enabled == false`.
    pub web_url: Option<String>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            config: Config::load(),
            image_viewer: ImageViewer::default(),
            file_tree: FileTree::default(),
            slideshow: Slideshow::default(),
            current_tags: Vec::new(),
            tags_modified: false,
            new_tag_input: String::new(),
            slideshow_dialog: SlideshowDialog::default(),
            slideshow_dir: None,
            status_message: String::new(),
            was_left_sidebar_open: false,
            was_right_sidebar_open: false,
            was_slideshow_dialog_open: false,
            current_texture: None,
            current_texture_path: None,
            web_url: None,
        }
    }

    /// Opens an image or sets the file-tree root from any path (file or directory).
    pub fn open_path(&mut self, path: PathBuf) {
        if !path.exists() {
            return;
        }
        if path.is_dir() {
            self.file_tree.set_root(&path);
            self.slideshow_dir = Some(path);
        } else if is_image_file(&path) {
            if let Some(parent) = path.parent() {
                self.file_tree.set_root(parent);
                self.slideshow_dir = Some(parent.to_path_buf());
            }
            self.open_image(path);
        }
    }

    pub fn open_image(&mut self, path: PathBuf) {
        self.image_viewer.open(&path);
        self.invalidate_texture();
        self.current_tags = metadata::load_tags(&path);
        self.tags_modified = false;
        self.status_message = format!("Opened: {}", path.display());
    }

    pub fn save_tags(&mut self) {
        let Some(path) = self.image_viewer.current_image.as_ref() else {
            return;
        };
        match metadata::save_tags(path, &self.current_tags) {
            Ok(()) => {
                self.tags_modified = false;
                self.status_message = "Tags saved".to_string();
            }
            Err(e) => {
                self.status_message = format!("Error saving tags: {}", e);
            }
        }
    }

    pub fn navigate(&mut self, direction: NavDirection) {
        if self.tags_modified && self.config.auto_save {
            self.save_tags();
        }
        match direction {
            NavDirection::Prev => self.image_viewer.prev(),
            NavDirection::Next => self.image_viewer.next(),
        }
        if let Some(path) = self.image_viewer.current_image.clone() {
            self.current_tags = metadata::load_tags(&path);
            self.tags_modified = false;
            self.invalidate_texture();
        }
    }

    /// Returns the viewer to a no-image-selected state without touching files. Bound to
    /// Esc by `input::handle_keyboard`. Does nothing if no image is currently displayed.
    pub fn close_current_image(&mut self) {
        if self.image_viewer.current_image.is_none() {
            return;
        }
        self.image_viewer.close();
        self.invalidate_texture();
        self.current_tags.clear();
        self.tags_modified = false;
        self.status_message = "Closed".to_string();
    }

    pub fn delete_current_image(&mut self) {
        let Some(path) = self.image_viewer.current_image.clone() else {
            return;
        };

        if let Err(e) = trash::delete(&path) {
            self.status_message = format!("Error deleting file: {}", e);
            return;
        }
        self.status_message = format!("Moved to trash: {}", path.display());

        match self.image_viewer.remove(&path) {
            Some(next) => self.open_image(next),
            None => {
                self.image_viewer.close();
                self.invalidate_texture();
                self.current_tags.clear();
                self.tags_modified = false;
            }
        }
    }

    pub fn handle_dropped_files(&mut self, ctx: &egui::Context) {
        let dropped: Vec<PathBuf> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .filter_map(|f| f.path.clone())
                .collect()
        });
        for path in dropped {
            self.open_path(path);
        }
    }

    pub fn update_slideshow(&mut self) {
        if let Some(path) = self
            .slideshow
            .update(self.config.slideshow_interval, self.config.slideshow_loop)
        {
            self.open_image(path);
        }
        if !self.slideshow.is_running && self.slideshow.completed_once {
            self.status_message = "Slideshow completed".to_string();
            self.slideshow.completed_once = false;
        }
    }

    /// Mutates the current tag list and triggers an autosave if enabled.
    pub fn modify_tags(&mut self, mutate: impl FnOnce(&mut Vec<String>)) {
        mutate(&mut self.current_tags);
        self.tags_modified = true;
        if self.config.auto_save {
            self.save_tags();
        }
    }

    pub fn current_image_path(&self) -> Option<&Path> {
        self.image_viewer.current_image.as_deref()
    }

    fn invalidate_texture(&mut self) {
        self.current_texture = None;
        self.current_texture_path = None;
    }
}
