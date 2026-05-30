use eframe::egui::{self, RichText};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::thumb_grid;
use crate::filter;
use crate::metadata::{self, is_metadata_supported};
use crate::scan_task::ScanTask;
use crate::state::{AppState, SlideshowListView};
use crate::thumbnail_cache::ThumbnailCache;

const THUMB_DIM: u32 = 96;
const ROW_HEIGHT_NAMES: f32 = 20.0;

/// Renders the slideshow setup UI inside whatever context is supplied — the caller
/// decides whether that context is a separate OS viewport or the main window.
pub fn draw_slideshow_dialog(
    ctx: &egui::Context,
    state: &mut AppState,
    thumbs: &mut ThumbnailCache,
) {
    // Stream in any results from an in-progress recursive scan before drawing.
    poll_scan(state, ctx);
    refresh_cache_on_dir_change(state, thumbs);

    let mut start_clicked = false;
    let mut cancel_clicked = false;

    egui::CentralPanel::default().show(ctx, |ui| {
        ui.heading("▶ Slideshow Setup");
        draw_source_picker(ui, state, thumbs);

        ui.separator();
        draw_hotkey_chips(ui, state);
        ui.separator();
        draw_free_word(ui, state);
        ui.separator();
        draw_view_toggle(ui, &mut state.slideshow_dialog.view);
        ui.separator();

        if let Some(scan) = state.slideshow_dialog.scan.as_ref() {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(scan.progress_label());
            });
        } else {
            ui.label(format!(
                "Matched: {} / {}",
                state.slideshow_dialog.filtered.len(),
                state.slideshow_dialog.tag_cache.len()
            ));
        }

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

/// Auto-loads the current directory's top-level images when it changes. Skipped
/// while a recursive base folder is selected — in that mode the cache is owned by
/// the background scan instead of the current directory.
fn refresh_cache_on_dir_change(state: &mut AppState, thumbs: &mut ThumbnailCache) {
    if state.slideshow_dialog.base_dir.is_some() {
        return;
    }
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

/// Source selector: switch between the current directory (top level only) and a
/// recursive base folder that includes every subfolder.
fn draw_source_picker(ui: &mut egui::Ui, state: &mut AppState, thumbs: &mut ThumbnailCache) {
    ui.horizontal(|ui| {
        if ui.button("📁 Base folder (recursive)...").clicked() {
            if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                start_recursive_scan(state, thumbs, dir);
            }
        }
        if state.slideshow_dialog.base_dir.is_some()
            && ui.button("✕ Use current folder").clicked()
        {
            clear_base(state, thumbs);
        }
    });

    let label = match state.slideshow_dialog.base_dir.as_ref() {
        Some(base) => format!("Source: {} (recursive)", base.display()),
        None => format!(
            "Directory: {}",
            state
                .slideshow_dir
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "(none)".into())
        ),
    };
    ui.label(label);
}

/// Starts a recursive background scan of `base`, switching the dialog into
/// recursive mode. Results stream in via [`poll_scan`].
fn start_recursive_scan(state: &mut AppState, thumbs: &mut ThumbnailCache, base: PathBuf) {
    thumbs.clear();
    let dialog = &mut state.slideshow_dialog;
    dialog.base_dir = Some(base.clone());
    dialog.tag_cache.clear();
    dialog.filtered.clear();
    dialog.scan = Some(ScanTask::spawn(base));
    state.status_message = "Scanning…".to_string();
}

/// Reverts to current-directory mode. Resetting `last_dir` forces the next
/// `refresh_cache_on_dir_change` to reload the current folder's top level.
fn clear_base(state: &mut AppState, thumbs: &mut ThumbnailCache) {
    thumbs.clear();
    let dialog = &mut state.slideshow_dialog;
    dialog.base_dir = None;
    dialog.scan = None;
    dialog.tag_cache.clear();
    dialog.filtered.clear();
    dialog.last_dir = None;
}

/// Drains the active recursive scan into the tag cache and refreshes the filter,
/// keeping the window repainting until the scan completes.
fn poll_scan(state: &mut AppState, ctx: &egui::Context) {
    let mut loaded = Vec::new();
    let (changed, done) = {
        let Some(scan) = state.slideshow_dialog.scan.as_mut() else {
            return;
        };
        let changed = scan.drain(|path, tags| loaded.push((path, tags)));
        (changed, scan.done)
    };

    for (path, tags) in loaded {
        state.slideshow_dialog.tag_cache.insert(path, tags);
    }
    if changed {
        recompute_filtered(state);
    }

    if done {
        let count = state.slideshow_dialog.tag_cache.len();
        state.slideshow_dialog.scan = None;
        state.status_message = format!("Scanned {} image(s)", count);
        // Persist the freshly-populated tag cache so the next scan is near-instant.
        crate::metadata::flush_cache();
    } else {
        ctx.request_repaint();
    }
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
    let total = state.slideshow_dialog.filtered.len();
    // In recursive mode, label rows with their path relative to the base so the
    // subfolder is visible; otherwise the bare file name is enough.
    let base = state.slideshow_dialog.base_dir.as_deref();

    match view {
        SlideshowListView::Names => {
            egui::ScrollArea::vertical()
                .max_height(height)
                .auto_shrink([false, false])
                .show_rows(ui, ROW_HEIGHT_NAMES, total, |ui, row_range| {
                    for idx in row_range {
                        draw_name_row(ui, &state.slideshow_dialog.filtered[idx], base);
                    }
                });
        }
        SlideshowListView::Thumbnails => {
            thumb_grid::thumbnail_grid(
                ui,
                thumbs,
                &state.slideshow_dialog.filtered,
                base,
                height,
                THUMB_DIM,
            );
        }
    }
}

fn draw_name_row(ui: &mut egui::Ui, path: &Path, base: Option<&Path>) {
    let label = base
        .and_then(|b| path.strip_prefix(b).ok())
        .map(|rel| rel.display().to_string())
        .unwrap_or_else(|| {
            path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string()
        });
    ui.label(label);
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
