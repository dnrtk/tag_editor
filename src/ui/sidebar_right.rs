use eframe::egui::{self, Key};

use crate::metadata;
use crate::state::AppState;

pub fn draw_right_sidebar(ui: &mut egui::Ui, state: &mut AppState) {
    draw_header(ui, state);
    ui.separator();
    draw_tag_list(ui, state);
    ui.separator();
    draw_tag_input(ui, state);
    ui.separator();
    draw_save_button(ui, state);
    ui.separator();
    if ui
        .checkbox(&mut state.config.auto_save, "Auto-save on hotkey")
        .changed()
    {
        state.config.save();
    }
    ui.separator();
    draw_hotkey_summary(ui, state);
}

fn draw_header(ui: &mut egui::Ui, state: &mut AppState) {
    ui.horizontal(|ui| {
        ui.heading("🏷 Tags");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let label = if state.config.right_sidebar_docked {
                "⏏ Float"
            } else {
                "📌 Dock"
            };
            if ui.small_button(label).clicked() {
                state.config.right_sidebar_docked = !state.config.right_sidebar_docked;
                state.config.save();
            }
        });
    });
}

fn draw_tag_list(ui: &mut egui::Ui, state: &mut AppState) {
    let scroll_height = (ui.available_height() - 100.0).max(0.0);
    egui::ScrollArea::vertical()
        .max_height(scroll_height)
        .show(ui, |ui| {
            let mut to_remove: Option<String> = None;
            for tag in &state.current_tags {
                ui.horizontal(|ui| {
                    ui.label(format!("• {}", tag));
                    if ui.small_button("✕").clicked() {
                        to_remove = Some(tag.clone());
                    }
                });
            }
            if let Some(tag) = to_remove {
                state.modify_tags(|tags| metadata::remove_tag(tags, &tag));
            }
        });
}

fn draw_tag_input(ui: &mut egui::Ui, state: &mut AppState) {
    ui.horizontal(|ui| {
        ui.label("Add:");
        let response = ui.text_edit_singleline(&mut state.new_tag_input);
        let submitted = response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
        let plus_clicked = ui.small_button("+").clicked();
        if (submitted || plus_clicked) && !state.new_tag_input.trim().is_empty() {
            let tag = std::mem::take(&mut state.new_tag_input);
            state.modify_tags(|tags| metadata::add_tag(tags, &tag));
        }
    });
}

fn draw_save_button(ui: &mut egui::Ui, state: &mut AppState) {
    let button = egui::Button::new("💾 Save (Ctrl+S)");
    if ui.add_enabled(state.tags_modified, button).clicked() {
        state.save_tags();
    }
}

fn draw_hotkey_summary(ui: &mut egui::Ui, state: &AppState) {
    ui.collapsing("⌨ Hotkeys", |ui| {
        let mut keys: Vec<&String> = state.config.hotkey_tags.keys().collect();
        keys.sort();

        if keys.is_empty() {
            ui.label("(No hotkeys configured)");
        } else {
            for key in keys {
                if let Some(tag) = state.config.hotkey_tags.get(key) {
                    ui.horizontal(|ui| {
                        ui.label(format!("[{}]:", key));
                        ui.label(tag);
                    });
                }
            }
        }
        ui.separator();
        ui.label("ℹ Edit settings.json to configure hotkeys");
    });
}
