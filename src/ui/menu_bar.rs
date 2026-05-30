use eframe::egui;

use crate::metadata::is_image_file;
use crate::state::AppState;

pub fn draw_menu_bar(ui: &mut egui::Ui, state: &mut AppState) {
    egui::menu::bar(ui, |ui| {
        ui.menu_button("File", |ui| file_menu(ui, state));
        ui.menu_button("View", |ui| view_menu(ui, state));
        ui.menu_button("Slideshow", |ui| slideshow_menu(ui, state));
        ui.menu_button("Search", |ui| search_menu(ui, state));
        ui.menu_button("Settings", |ui| settings_menu(ui, state));

        // Right-aligned utility buttons.
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            draw_web_button(ui, state);
        });
    });
}

fn draw_web_button(ui: &mut egui::Ui, state: &mut AppState) {
    let Some(url) = state.web_url.clone() else {
        let resp = ui.add_enabled(false, egui::Button::new("🌐 Web UI"));
        resp.on_hover_text("Web server is disabled or failed to start");
        return;
    };

    let response = ui.button("🌐 Open in Browser");
    let response = response.on_hover_text(format!("{} (also reachable on the LAN)", url));
    if response.clicked() {
        if let Err(e) = webbrowser::open(&url) {
            state.status_message = format!("Browser open failed: {}", e);
        } else {
            state.status_message = format!("Opened: {}", url);
        }
    }
}

fn file_menu(ui: &mut egui::Ui, state: &mut AppState) {
    if ui.button("Open Image...").clicked() {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Images", &["png", "jpg", "jpeg", "gif", "webp", "bmp"])
            .pick_file()
        {
            if is_image_file(&path) {
                state.open_path(path);
            }
        }
        ui.close_menu();
    }
    if ui.button("Open Folder...").clicked() {
        if let Some(path) = rfd::FileDialog::new().pick_folder() {
            state.open_path(path);
        }
        ui.close_menu();
    }
    ui.separator();
    if ui.button("Save Tags (Ctrl+S)").clicked() {
        state.save_tags();
        ui.close_menu();
    }
}

fn view_menu(ui: &mut egui::Ui, state: &mut AppState) {
    let mut changed = false;
    changed |= ui
        .checkbox(&mut state.config.show_left_sidebar, "Files Window (Ctrl+F)")
        .changed();
    changed |= ui
        .checkbox(&mut state.config.show_right_sidebar, "Tags Window (Ctrl+T)")
        .changed();
    if changed {
        state.config.save();
    }
}

fn slideshow_menu(ui: &mut egui::Ui, state: &mut AppState) {
    if ui.button("Start Slideshow...").clicked() {
        state.slideshow_dialog.open = true;
        // Force a fresh dir comparison so the tag cache reloads when reopened.
        state.slideshow_dialog.last_dir = None;
        ui.close_menu();
    }
    if state.slideshow.is_running && ui.button("Stop Slideshow").clicked() {
        state.slideshow.stop();
        ui.close_menu();
    }
}

fn search_menu(ui: &mut egui::Ui, state: &mut AppState) {
    if ui.button("Search Images...").clicked() {
        // Pre-fill the base folder with the current file-tree root for convenience;
        // the user can still pick any other folder inside the window.
        if state.search_dialog.base_dir.is_none() {
            state.search_dialog.base_dir = state
                .file_tree
                .root
                .as_ref()
                .map(|r| r.path.clone())
                .or_else(|| state.slideshow_dir.clone());
        }
        state.search_dialog.open = true;
        ui.close_menu();
    }
}

fn settings_menu(ui: &mut egui::Ui, state: &mut AppState) {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.label("Slideshow interval (sec):");
        changed |= ui
            .add(egui::DragValue::new(&mut state.config.slideshow_interval).range(0.5..=60.0))
            .changed();
    });
    changed |= ui
        .checkbox(&mut state.config.slideshow_loop, "Loop slideshow")
        .changed();
    ui.separator();
    changed |= ui
        .checkbox(&mut state.config.auto_save, "Auto-save on hotkey")
        .changed();
    ui.separator();

    ui.label(egui::RichText::new("Web server").strong());
    changed |= ui
        .checkbox(&mut state.config.web_enabled, "Enable web server")
        .changed();
    ui.horizontal(|ui| {
        ui.label("Port:");
        // 1024..=65535 keeps us out of the privileged range on most OSes.
        let resp = ui.add_enabled(
            state.config.web_enabled,
            egui::DragValue::new(&mut state.config.web_port).range(1024..=65535),
        );
        if resp.changed() {
            changed = true;
        }
    });
    if let Some(url) = state.web_url.as_deref() {
        ui.label(format!("Listening: {}", url));
    } else if state.config.web_enabled {
        ui.colored_label(egui::Color32::YELLOW, "Server not running");
    }

    if changed {
        state.config.save();
    }
}
