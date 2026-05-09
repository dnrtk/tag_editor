use eframe::egui::{self, Color32, RichText};

use crate::state::AppState;

pub fn draw_status_bar(ui: &mut egui::Ui, state: &AppState) {
    ui.horizontal(|ui| {
        if let Some(path) = state.current_image_path() {
            let file_name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");
            ui.label(format!(
                "{} ({}/{})",
                file_name,
                state.image_viewer.current_index + 1,
                state.image_viewer.total_images()
            ));
            ui.separator();
        }

        if state.slideshow.is_running {
            ui.label(RichText::new("▶ Slideshow").color(Color32::GREEN));
        }
        if state.tags_modified {
            ui.label(RichText::new("● Modified").color(Color32::YELLOW));
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(&state.status_message);
        });
    });
}
