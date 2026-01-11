#![windows_subsystem = "windows"]

mod app;
mod config;
mod file_tree;
mod image_viewer;
mod slideshow;
mod tag_manager;

use app::TagEditorApp;
use eframe::egui;
use std::path::PathBuf;

fn main() -> eframe::Result<()> {
    // コマンドライン引数から初期パスを取得（exeへのD&Dで渡される）
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
