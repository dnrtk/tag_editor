use eframe::egui::{self, Color32, Vec2};
use std::collections::BTreeMap;
use std::path::Path;

use crate::state::AppState;

pub fn draw_center(ui: &mut egui::Ui, state: &mut AppState) {
    let Some(path) = state.image_viewer.current_image.clone() else {
        ui.centered_and_justified(|ui| {
            ui.heading("🖼 Drop an image or folder here");
        });
        return;
    };

    ensure_texture_loaded(ui.ctx(), state, &path);

    if let Some(tex) = state.current_texture.clone() {
        let available = ui.available_size();
        let response = ui.add(egui::Image::new(&tex).fit_to_exact_size(available));
        draw_hotkey_overlay(ui, state, response.rect);
    } else {
        ui.centered_and_justified(|ui| {
            ui.heading("🖼 Failed to load image");
        });
    }
}

fn ensure_texture_loaded(ctx: &egui::Context, state: &mut AppState, path: &Path) {
    if state.current_texture_path.as_deref() == Some(path) && state.current_texture.is_some() {
        return;
    }

    state.current_texture = load_texture(ctx, path);
    state.current_texture_path = state.current_texture.is_some().then(|| path.to_path_buf());
}

fn load_texture(ctx: &egui::Context, path: &Path) -> Option<egui::TextureHandle> {
    let img = image::open(path).ok()?.to_rgba8();
    let (w, h) = img.dimensions();
    let pixels = img.into_raw();
    let color_image =
        egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &pixels);
    Some(ctx.load_texture(
        path.display().to_string(),
        color_image,
        egui::TextureOptions::default(),
    ))
}

fn draw_hotkey_overlay(ui: &mut egui::Ui, state: &AppState, rect: egui::Rect) {
    // Tag → sorted keys map. BTreeMap groups keys per tag while keeping deterministic order.
    let mut tag_to_keys: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for (key, tag) in &state.config.hotkey_tags {
        tag_to_keys.entry(tag.as_str()).or_default().push(key.as_str());
    }
    for keys in tag_to_keys.values_mut() {
        keys.sort();
    }

    // Visit current tags in their original order so the overlay matches the sidebar.
    let entries: Vec<(&str, &Vec<&str>)> = state
        .current_tags
        .iter()
        .filter_map(|tag| tag_to_keys.get_key_value(tag.as_str()).map(|(t, k)| (*t, k)))
        .collect();

    if entries.is_empty() {
        return;
    }

    const PADDING: Vec2 = Vec2::new(8.0, 4.0);
    const SPACING: f32 = 8.0;
    const FONT_SIZE: f32 = 16.0;
    const CORNER: f32 = 4.0;

    let painter = ui.painter();
    let start = rect.min + Vec2::new(10.0, 10.0);
    let mut cursor = start;

    for (tag, keys) in entries {
        let label = format_label(keys, tag);
        let galley = painter.layout_no_wrap(label, egui::FontId::proportional(FONT_SIZE), Color32::WHITE);
        let box_size = galley.size() + PADDING * 2.0;

        if cursor.x + box_size.x > rect.max.x {
            cursor.x = start.x;
            cursor.y += box_size.y + SPACING;
        }

        let bg = egui::Rect::from_min_size(cursor, box_size);
        painter.rect_filled(bg, CORNER, Color32::from_rgba_unmultiplied(0, 0, 0, 180));
        painter.rect_stroke(bg, CORNER, egui::Stroke::new(1.0, Color32::from_gray(128)));
        painter.galley(cursor + PADDING, galley, Color32::WHITE);

        cursor.x += box_size.x + SPACING;
    }
}

fn format_label(keys: &[&str], tag: &str) -> String {
    let mut s = String::with_capacity(keys.len() * 3 + tag.len() + 1);
    for key in keys {
        s.push('[');
        s.push_str(key);
        s.push(']');
    }
    s.push(' ');
    s.push_str(tag);
    s
}
