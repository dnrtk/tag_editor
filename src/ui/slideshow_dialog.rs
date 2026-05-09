use eframe::egui::{self, Color32, RichText};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::filter;
use crate::metadata::{self, is_metadata_supported};
use crate::state::{AppState, SlideshowListView};
use crate::thumbnail_cache::ThumbnailCache;

const THUMB_DIM: u32 = 96;
const ROW_HEIGHT_NAMES: f32 = 20.0;
const ROW_HEIGHT_THUMBS: f32 = 110.0;

/// Renders the slideshow setup UI inside whatever context is supplied — the caller
/// decides whether that context is a separate OS viewport or the main window.
pub fn draw_slideshow_dialog(
    ctx: &egui::Context,
    state: &mut AppState,
    thumbs: &mut ThumbnailCache,
) {
    refresh_cache_on_dir_change(state, thumbs);

    let dir_label = state
        .slideshow_dir
        .as_ref()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "(none)".into());

    let mut start_clicked = false;
    let mut cancel_clicked = false;

    egui::CentralPanel::default().show(ctx, |ui| {
        ui.heading("▶ Slideshow Setup");
        ui.label(format!("Directory: {}", dir_label));

        ui.separator();
        draw_hotkey_chips(ui, state);
        ui.separator();
        draw_free_word(ui, state);
        ui.separator();
        draw_view_toggle(ui, &mut state.slideshow_dialog.view);
        ui.separator();

        ui.label(format!(
            "Matched: {} / {}",
            state.slideshow_dialog.filtered.len(),
            state.slideshow_dialog.tag_cache.len()
        ));

        ui.separator();

        // Reserve space for the bottom buttons so the list area scrolls cleanly.
        let buttons_h = 40.0;
        let list_h = (ui.available_height() - buttons_h).max(80.0);
        draw_filtered_list(ui, thumbs, state, list_h);

        ui.separator();
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    !state.slideshow_dialog.filtered.is_empty(),
                    egui::Button::new("▶ Start"),
                )
                .clicked()
            {
                start_clicked = true;
            }
            if ui.button("Cancel").clicked() {
                cancel_clicked = true;
            }
        });
    });

    if cancel_clicked {
        state.slideshow_dialog.open = false;
    }
    if start_clicked {
        start_slideshow(state);
    }
}

fn refresh_cache_on_dir_change(state: &mut AppState, thumbs: &mut ThumbnailCache) {
    let dir = state.slideshow_dir.clone();
    if dir == state.slideshow_dialog.last_dir {
        return;
    }
    thumbs.clear();
    state.slideshow_dialog.tag_cache.clear();
    if let Some(d) = dir.as_ref() {
        load_tag_cache(d, &mut state.slideshow_dialog.tag_cache);
    }
    state.slideshow_dialog.last_dir = dir;
    recompute_filtered(state);
}

fn draw_hotkey_chips(ui: &mut egui::Ui, state: &mut AppState) {
    ui.label(RichText::new("Hotkey tags").strong());
    let mut keys: Vec<&String> = state.config.hotkey_tags.keys().collect();
    keys.sort();

    if keys.is_empty() {
        ui.label("(no hotkeys configured)");
        return;
    }

    let mut changed = false;
    ui.horizontal_wrapped(|ui| {
        for key in keys {
            let Some(tag) = state.config.hotkey_tags.get(key) else {
                continue;
            };
            let selected = state.slideshow_dialog.selected_tags.contains(tag);
            let response = ui.selectable_label(selected, format!("[{}] {}", key, tag));
            if response.clicked() {
                if selected {
                    state.slideshow_dialog.selected_tags.remove(tag);
                } else {
                    state.slideshow_dialog.selected_tags.insert(tag.clone());
                }
                changed = true;
            }
        }
    });
    if changed {
        recompute_filtered(state);
    }
}

fn draw_free_word(ui: &mut egui::Ui, state: &mut AppState) {
    ui.horizontal(|ui| {
        ui.label("Free word:");
        let response = ui.text_edit_singleline(&mut state.slideshow_dialog.free_word);
        if response.changed() {
            recompute_filtered(state);
        }
        if !state.slideshow_dialog.free_word.is_empty() && ui.small_button("✕").clicked() {
            state.slideshow_dialog.free_word.clear();
            recompute_filtered(state);
        }
    });
}

fn draw_view_toggle(ui: &mut egui::Ui, view: &mut SlideshowListView) {
    ui.horizontal(|ui| {
        ui.label("View:");
        ui.selectable_value(view, SlideshowListView::Names, "Names");
        ui.selectable_value(view, SlideshowListView::Thumbnails, "Thumbnails");
    });
}

fn draw_filtered_list(
    ui: &mut egui::Ui,
    thumbs: &mut ThumbnailCache,
    state: &AppState,
    height: f32,
) {
    let view = state.slideshow_dialog.view;
    let row_height = match view {
        SlideshowListView::Names => ROW_HEIGHT_NAMES,
        SlideshowListView::Thumbnails => ROW_HEIGHT_THUMBS,
    };
    let total = state.slideshow_dialog.filtered.len();

    egui::ScrollArea::vertical()
        .max_height(height)
        .show_rows(ui, row_height, total, |ui, row_range| {
            for idx in row_range {
                let path = &state.slideshow_dialog.filtered[idx];
                draw_row(ui, thumbs, path, view);
            }
        });
}

fn draw_row(
    ui: &mut egui::Ui,
    thumbs: &mut ThumbnailCache,
    path: &Path,
    view: SlideshowListView,
) {
    match view {
        SlideshowListView::Names => {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            ui.label(name);
        }
        SlideshowListView::Thumbnails => {
            // Thumbnail-only: filename is intentionally omitted to reduce noise.
            // Hover for the path tooltip if needed.
            let max = THUMB_DIM as f32;
            let response = if let Some(tex) = thumbs.get(ui.ctx(), path, THUMB_DIM) {
                ui.add(egui::Image::new(tex).fit_to_exact_size(egui::vec2(max, max)))
            } else {
                ui.add_sized(
                    egui::vec2(max, max),
                    egui::Label::new(egui::RichText::new("[no preview]").color(Color32::DARK_GRAY)),
                )
            };
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                response.on_hover_text(name);
            }
        }
    }
}

fn load_tag_cache(dir: &Path, cache: &mut HashMap<PathBuf, Vec<String>>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if is_metadata_supported(&path) {
            let tags = metadata::load_tags(&path);
            cache.insert(path, tags);
        }
    }
}

fn recompute_filtered(state: &mut AppState) {
    let dialog = &mut state.slideshow_dialog;
    let mut paths: Vec<PathBuf> = dialog
        .tag_cache
        .iter()
        .filter(|(_, tags)| filter::matches(tags, &dialog.selected_tags, &dialog.free_word))
        .map(|(p, _)| p.clone())
        .collect();
    paths.sort();
    dialog.filtered = paths;
}

fn start_slideshow(state: &mut AppState) {
    if state.slideshow_dialog.filtered.is_empty() {
        state.status_message = "No images match the current filter".to_string();
        return;
    }
    let images = state.slideshow_dialog.filtered.clone();
    state.slideshow.start(images);
    if let Some(path) = state.slideshow.current_image().cloned() {
        state.open_image(path);
    }
    state.status_message = "Slideshow started".to_string();
    state.slideshow_dialog.open = false;
}
