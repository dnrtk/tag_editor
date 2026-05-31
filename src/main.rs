// The `windows_subsystem = "windows"` attribute hides the console for the desktop
// GUI build only; the headless server build keeps a console so its logs are visible.
#![cfg_attr(feature = "gui", windows_subsystem = "windows")]
// In the headless build, GUI-only helpers (tag mutators, server shutdown) are
// legitimately unused; silence the resulting dead-code noise for that build only.
#![cfg_attr(not(feature = "gui"), allow(dead_code))]

// Server core — always compiled, GUI-independent and pure Rust.
mod config;
mod filter;
mod metadata;
mod search;
mod web;

// Desktop GUI — compiled only with the default `gui` feature, so the headless
// server build carries none of the eframe/winit/GTK dependencies.
#[cfg(feature = "gui")]
mod app;
#[cfg(feature = "gui")]
mod file_tree;
#[cfg(feature = "gui")]
mod image_viewer;
#[cfg(feature = "gui")]
mod input;
#[cfg(feature = "gui")]
mod scan_task;
#[cfg(feature = "gui")]
mod slideshow;
#[cfg(feature = "gui")]
mod state;
#[cfg(feature = "gui")]
mod thumbnail_cache;
#[cfg(feature = "gui")]
mod ui;

fn main() {
    // With the GUI compiled in, run the desktop app unless `--server`/`--headless`
    // is requested. The headless build (no `gui` feature) always runs the server.
    #[cfg(feature = "gui")]
    {
        let headless = std::env::args().any(|a| a == "--server" || a == "--headless");
        if !headless {
            if let Err(e) = run_gui() {
                eprintln!("GUI error: {e}");
                std::process::exit(1);
            }
            return;
        }
    }
    run_server();
}

#[cfg(feature = "gui")]
fn run_gui() -> eframe::Result<()> {
    use app::TagEditorApp;
    use eframe::egui;
    use std::path::PathBuf;

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

/// Headless entry point: start the embedded web server and block forever. Used by
/// the `--server` flag and by the GUI-less build (e.g. a Raspberry Pi / NAS
/// container). The tag cache is flushed periodically so it survives restarts when
/// its directory is persisted.
fn run_server() {
    use std::time::Duration;

    let config = config::Config::load();
    if !config.web_enabled {
        eprintln!("Web server is disabled (web_enabled = false). Enable it in the config and retry.");
        std::process::exit(1);
    }

    match web::spawn(config) {
        Ok(handle) => {
            println!("Tag Editor web server listening on {}", handle.bind);
            println!("Open: {}", handle.local_url());
            // The server runs on its own thread; park the main thread and flush the
            // tag cache occasionally so a mounted cache directory persists it.
            loop {
                std::thread::sleep(Duration::from_secs(300));
                metadata::flush_cache();
            }
        }
        Err(e) => {
            eprintln!("Failed to start web server: {e}");
            std::process::exit(1);
        }
    }
}
