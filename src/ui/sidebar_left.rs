use eframe::egui::{self, RichText};

use crate::file_tree::FileNode;
use crate::state::AppState;

pub fn draw_left_sidebar(ui: &mut egui::Ui, state: &mut AppState) {
    draw_header(ui, state);
    ui.separator();
    egui::ScrollArea::vertical()
        // Hide the horizontal bar; long names are truncated rather than scrolled.
        .auto_shrink([false, false])
        .show(ui, |ui| {
            // Apply truncation to all text in the tree. The sidebar is resizable so
            // the user can widen it if a name they care about is hidden under "...".
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);

            let Some(root) = state.file_tree.root.clone() else {
                ui.label("Drop a folder or image here");
                return;
            };
            draw_node(ui, state, &root);
        });
}

fn draw_header(ui: &mut egui::Ui, state: &mut AppState) {
    ui.horizontal(|ui| {
        ui.heading("📁 Files");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let label = if state.config.left_sidebar_docked {
                "⏏ Float"
            } else {
                "📌 Dock"
            };
            if ui.small_button(label).clicked() {
                state.config.left_sidebar_docked = !state.config.left_sidebar_docked;
                state.config.save();
            }
        });
    });
}

fn draw_node(ui: &mut egui::Ui, state: &mut AppState, node: &FileNode) {
    if node.is_dir {
        draw_directory(ui, state, node);
    } else {
        draw_file(ui, state, node);
    }
}

fn draw_directory(ui: &mut egui::Ui, state: &mut AppState, node: &FileNode) {
    let is_expanded = state.file_tree.is_expanded(&node.path);
    let icon = if is_expanded { "📂" } else { "📁" };

    let response = egui::CollapsingHeader::new(format!("{} {}", icon, node.name))
        .open(Some(is_expanded))
        .show(ui, |ui| {
            for child in &node.children {
                draw_node(ui, state, child);
            }
        });

    if response.header_response.clicked() {
        state.file_tree.toggle_expanded(&node.path);
    }
}

fn draw_file(ui: &mut egui::Ui, state: &mut AppState, node: &FileNode) {
    let is_current = state
        .image_viewer
        .current_image
        .as_deref()
        .is_some_and(|p| p == node.path);

    let label = if is_current {
        RichText::new(format!("🖼 {}", node.name)).strong()
    } else {
        RichText::new(format!("  {}", node.name))
    };

    if ui.selectable_label(is_current, label).clicked() {
        state.open_image(node.path.clone());
    }
}
