use eframe::egui::{self, RichText};
use std::path::Path;

use super::thumb_grid;
use crate::filter;
use crate::scan_task::ScanTask;
use crate::search;
use crate::state::{AppState, SlideshowListView};
use crate::thumbnail_cache::ThumbnailCache;

const THUMB_DIM: u32 = 96;
const ROW_HEIGHT_NAMES: f32 = 20.0;

/// Renders the recursive-search window inside whatever context is supplied.
/// Mirrors the slideshow dialog's layout but searches a base folder
/// recursively and can bulk-export the filtered results.
pub fn draw_search_dialog(
    ctx: &egui::Context,
    state: &mut AppState,
    thumbs: &mut ThumbnailCache,
) {
    let mut close_clicked = false;

    // Pull any results produced by the background scan before drawing, so the list
    // reflects the latest progress and the window keeps repainting while scanning.
    poll_scan(state, ctx);

    egui::CentralPanel::default().show(ctx, |ui| {
        ui.heading("🔍 Search Images");

        draw_base_picker(ui, state, thumbs);
        ui.separator();
        draw_hotkey_chips(ui, state);
        ui.separator();
        draw_free_word(ui, state);
        ui.separator();
        draw_view_toggle(ui, &mut state.search_dialog.view);
        ui.separator();

        if let Some(scan) = state.search_dialog.scan.as_ref() {
            // Active scan: a spinner plus a running count makes it obvious the app is
            // working (not frozen), even on a folder tree that takes seconds to read.
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(scan.progress_label());
            });
        } else {
            let scanned = state.search_dialog.scanned_dir.is_some();
            ui.label(format!(
                "Matched: {} / {}{}",
                state.search_dialog.filtered.len(),
                state.search_dialog.tag_cache.len(),
                if scanned { "" } else { "  (not scanned yet)" },
            ));
        }

        ui.separator();

        // Reserve room for the bottom action row so the list scrolls cleanly.
        let buttons_h = 40.0;
        let list_h = (ui.available_height() - buttons_h).max(80.0);
        draw_filtered_list(ui, thumbs, state, list_h);

        ui.separator();
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    !state.search_dialog.filtered.is_empty(),
                    egui::Button::new("📤 Export..."),
                )
                .clicked()
            {
                export_filtered(state);
            }
            if ui.button("Close").clicked() {
                close_clicked = true;
            }
        });
    });

    if close_clicked {
        state.search_dialog.open = false;
    }
}

fn draw_base_picker(ui: &mut egui::Ui, state: &mut AppState, thumbs: &mut ThumbnailCache) {
    let base_label = state
        .search_dialog
        .base_dir
        .as_ref()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "(none)".into());

    ui.horizontal(|ui| {
        if ui.button("📁 Base folder...").clicked() {
            if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                state.search_dialog.base_dir = Some(dir);
            }
        }
        // Disable the button while a scan is running so the user can't stack scans.
        let can_scan =
            state.search_dialog.base_dir.is_some() && state.search_dialog.scan.is_none();
        if ui
            .add_enabled(can_scan, egui::Button::new("🔍 Search"))
            .clicked()
        {
            scan_base(state, thumbs);
        }
    });
    ui.label(format!("Base: {}", base_label));
}

/// Kicks off a recursive scan of the base folder on a background thread. Results
/// stream into the tag cache via [`poll_scan`] so the UI never blocks on disk I/O.
fn scan_base(state: &mut AppState, thumbs: &mut ThumbnailCache) {
    let Some(base) = state.search_dialog.base_dir.clone() else {
        return;
    };
    thumbs.clear();
    let dialog = &mut state.search_dialog;
    dialog.tag_cache.clear();
    dialog.filtered.clear();
    dialog.scanned_dir = Some(base.clone());
    dialog.scan = Some(ScanTask::spawn(base));
    state.status_message = "Scanning…".to_string();
}

/// Drains the active background scan, folding any newly-loaded images into the
/// tag cache and refreshing the filtered list. Requests a repaint while the scan
/// is in flight so progress keeps updating, and clears the task when it finishes.
fn poll_scan(state: &mut AppState, ctx: &egui::Context) {
    let mut loaded = Vec::new();
    let (changed, done) = {
        let Some(scan) = state.search_dialog.scan.as_mut() else {
            return;
        };
        let changed = scan.drain(|path, tags| loaded.push((path, tags)));
        (changed, scan.done)
    };

    for (path, tags) in loaded {
        state.search_dialog.tag_cache.insert(path, tags);
    }
    if changed {
        recompute_filtered(state);
    }

    if done {
        let count = state.search_dialog.tag_cache.len();
        state.search_dialog.scan = None;
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
            let selected = state.search_dialog.selected_tags.contains(tag);
            let response = ui.selectable_label(selected, format!("[{}] {}", key, tag));
            if response.clicked() {
                if selected {
                    state.search_dialog.selected_tags.remove(tag);
                } else {
                    state.search_dialog.selected_tags.insert(tag.clone());
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
        let response = ui.text_edit_singleline(&mut state.search_dialog.free_word);
        if response.changed() {
            recompute_filtered(state);
        }
        if !state.search_dialog.free_word.is_empty() && ui.small_button("✕").clicked() {
            state.search_dialog.free_word.clear();
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
    let total = state.search_dialog.filtered.len();
    // Show the path relative to the scanned base so the subfolder is visible.
    let base = state.search_dialog.scanned_dir.as_deref();

    match state.search_dialog.view {
        SlideshowListView::Names => {
            egui::ScrollArea::vertical()
                .max_height(height)
                .auto_shrink([false, false])
                .show_rows(ui, ROW_HEIGHT_NAMES, total, |ui, row_range| {
                    for idx in row_range {
                        draw_name_row(ui, &state.search_dialog.filtered[idx], base);
                    }
                });
        }
        SlideshowListView::Thumbnails => {
            thumb_grid::thumbnail_grid(
                ui,
                thumbs,
                &state.search_dialog.filtered,
                base,
                height,
                THUMB_DIM,
            );
        }
    }
}

fn draw_name_row(ui: &mut egui::Ui, path: &Path, base: Option<&Path>) {
    let rel = base
        .and_then(|b| path.strip_prefix(b).ok())
        .unwrap_or(path)
        .display()
        .to_string();
    ui.label(&rel).on_hover_text(path.display().to_string());
}

fn recompute_filtered(state: &mut AppState) {
    let dialog = &mut state.search_dialog;
    let mut paths: Vec<_> = dialog
        .tag_cache
        .iter()
        .filter(|(_, tags)| filter::matches(tags, &dialog.selected_tags, &dialog.free_word))
        .map(|(p, _)| p.clone())
        .collect();
    paths.sort();
    dialog.filtered = paths;
}

/// Prompts for a destination folder and copies the filtered results there,
/// preserving each file's path relative to the scanned base folder.
fn export_filtered(state: &mut AppState) {
    if state.search_dialog.filtered.is_empty() {
        return;
    }
    let Some(base) = state.search_dialog.scanned_dir.clone() else {
        state.status_message = "Scan a base folder before exporting".to_string();
        return;
    };
    let Some(dest) = rfd::FileDialog::new().pick_folder() else {
        return;
    };

    let files = state.search_dialog.filtered.clone();
    let (copied, errors) = search::export_preserving_structure(&base, &files, &dest);
    state.status_message = if errors.is_empty() {
        format!("Exported {} file(s) to {}", copied, dest.display())
    } else {
        format!(
            "Exported {} file(s), {} failed (first: {})",
            copied,
            errors.len(),
            errors.first().map(String::as_str).unwrap_or("")
        )
    };
}
