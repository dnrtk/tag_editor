#![windows_subsystem = "windows"]

mod app;
mod config;
mod file_tree;
mod filter;
mod image_viewer;
mod input;
mod metadata;
mod slideshow;
mod state;
mod thumbnail_cache;
mod ui;
mod web;

use app::TagEditorApp;
use eframe::egui;
use std::path::PathBuf;

fn main() -> eframe::Result<()> {
    // The first CLI argument is the path the OS hands us when the user drops a file
    // onto the executable.
    let initial_path: Option<PathBuf> = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .filter(|p| p.exists());

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 720.0])
            .with_min_inner_size([200.0, 150.0])
            .with_drag_and_drop(true),
        ..Default::default()
    };

    eframe::run_native(
        "Image Tag Editor",
        options,
        Box::new(move |cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            Ok(Box::new(TagEditorApp::new(cc, initial_path)))
        }),
    )
}
