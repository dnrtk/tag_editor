use eframe::egui;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// In-memory cache of small preview textures, keyed by absolute image path.
/// Loads on demand and never evicts — the dialogs that use this stay short-lived
/// (slideshow dialog) so unbounded growth in practice is bounded by directory size.
#[derive(Default)]
pub struct ThumbnailCache {
    entries: HashMap<PathBuf, Option<egui::TextureHandle>>,
}

impl ThumbnailCache {
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Returns a handle for `path`, loading and caching it on first access.
    /// Returns `None` if the image cannot be decoded.
    pub fn get(
        &mut self,
        ctx: &egui::Context,
        path: &Path,
        max_dim: u32,
    ) -> Option<&egui::TextureHandle> {
        if !self.entries.contains_key(path) {
            let texture = load(ctx, path, max_dim);
            self.entries.insert(path.to_path_buf(), texture);
        }
        self.entries.get(path).and_then(Option::as_ref)
    }
}

fn load(ctx: &egui::Context, path: &Path, max_dim: u32) -> Option<egui::TextureHandle> {
    let img = image::open(path).ok()?.thumbnail(max_dim, max_dim).to_rgba8();
    let (w, h) = img.dimensions();
    let pixels = img.into_raw();
    let color_image =
        egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &pixels);
    Some(ctx.load_texture(
        format!("thumb:{}", path.display()),
        color_image,
        egui::TextureOptions::LINEAR,
    ))
}
