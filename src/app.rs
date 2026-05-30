use eframe::egui;
use std::path::PathBuf;

use crate::input;
use crate::state::AppState;
use crate::thumbnail_cache::ThumbnailCache;
use crate::ui;
use crate::web::WebHandle;

pub struct TagEditorApp {
    state: AppState,
    slideshow_thumbs: ThumbnailCache,
    search_thumbs: ThumbnailCache,
    /// Holds the running web server. Replaced on port change so the old listener stops
    /// before a new one binds. None when the server is disabled or failed to start.
    web_handle: Option<WebHandle>,
    /// Snapshot of the web settings as they were when the current `web_handle` was started.
    /// Used to detect changes and trigger a hot-restart.
    web_settings_snapshot: WebSettings,
}

#[derive(PartialEq, Eq, Clone)]
struct WebSettings {
    enabled: bool,
    port: u16,
}

impl WebSettings {
    fn from_config(c: &crate::config::Config) -> Self {
        Self {
            enabled: c.web_enabled,
            port: c.web_port,
        }
    }
}

impl TagEditorApp {
    pub fn new(cc: &eframe::CreationContext<'_>, initial_path: Option<PathBuf>) -> Self {
        install_japanese_font(&cc.egui_ctx);
        cc.egui_ctx.set_visuals(egui::Visuals::dark());

        let mut state = AppState::new();
        if let Some(path) = initial_path {
            state.open_path(path);
        }

        let mut app = Self {
            web_settings_snapshot: WebSettings::from_config(&state.config),
            state,
            slideshow_thumbs: ThumbnailCache::default(),
            search_thumbs: ThumbnailCache::default(),
            web_handle: None,
        };
        app.start_web_server();
        app
    }

    fn start_web_server(&mut self) {
        if !self.state.config.web_enabled {
            self.web_handle = None;
            self.state.web_url = None;
            return;
        }
        match crate::web::spawn(self.state.config.clone()) {
            Ok(handle) => {
                let url = handle.local_url();
                self.state.web_url = Some(url.clone());
                self.state.status_message = format!("Web UI: {}", url);
                self.web_handle = Some(handle);
            }
            Err(e) => {
                self.state.web_url = None;
                self.state.status_message = format!("Web server failed: {}", e);
                self.web_handle = None;
            }
        }
    }

    fn restart_web_server_if_needed(&mut self) {
        let current = WebSettings::from_config(&self.state.config);
        if current == self.web_settings_snapshot {
            return;
        }
        if let Some(handle) = self.web_handle.take() {
            handle.shutdown();
        }
        self.start_web_server();
        self.web_settings_snapshot = current;
    }
}

impl eframe::App for TagEditorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.state.handle_dropped_files(ctx);
        input::handle_keyboard(&mut self.state, ctx);
        self.state.update_slideshow();
        if self.state.slideshow.is_running {
            ctx.request_repaint();
        }

        self.restart_web_server_if_needed();

        self.draw_main_window(ctx);

        let main_rect = ctx
            .input(|i| i.viewport().outer_rect)
            .unwrap_or_else(|| ctx.input(|i| i.screen_rect()));

        if self.state.config.show_left_sidebar && !self.state.config.left_sidebar_docked {
            self.draw_floating_sidebar(ctx, Side::Left, main_rect);
        }
        if self.state.config.show_right_sidebar && !self.state.config.right_sidebar_docked {
            self.draw_floating_sidebar(ctx, Side::Right, main_rect);
        }
        if self.state.slideshow_dialog.open {
            self.draw_slideshow_dialog_viewport(ctx, main_rect);
        }
        if self.state.search_dialog.open {
            self.draw_search_dialog_viewport(ctx, main_rect);
        }

        let undocked_left =
            self.state.config.show_left_sidebar && !self.state.config.left_sidebar_docked;
        let undocked_right =
            self.state.config.show_right_sidebar && !self.state.config.right_sidebar_docked;
        self.state.was_left_sidebar_open = undocked_left;
        self.state.was_right_sidebar_open = undocked_right;
        self.state.was_slideshow_dialog_open = self.state.slideshow_dialog.open;
        self.state.was_search_dialog_open = self.state.search_dialog.open;
    }

    fn save(&mut self, _storage: &mut dyn eframe::Storage) {
        self.state.config.save();
        // Periodically persisted by eframe; also flushes the tag cache to disk.
        crate::metadata::flush_cache();
    }
}

#[derive(Clone, Copy)]
enum Side {
    Left,
    Right,
}

impl TagEditorApp {
    fn draw_main_window(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            ui::draw_menu_bar(ui, &mut self.state);
        });
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui::draw_status_bar(ui, &self.state);
        });

        if self.state.config.show_left_sidebar && self.state.config.left_sidebar_docked {
            self.draw_docked_sidebar(ctx, Side::Left);
        }
        if self.state.config.show_right_sidebar && self.state.config.right_sidebar_docked {
            self.draw_docked_sidebar(ctx, Side::Right);
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui::draw_center(ui, &mut self.state);
        });
    }

    fn draw_docked_sidebar(&mut self, ctx: &egui::Context, side: Side) {
        let (id, default_w, min_w, max_w) = match side {
            Side::Left => ("dock_left", self.state.config.left_dock_width, 180.0, 600.0),
            Side::Right => ("dock_right", self.state.config.right_dock_width, 180.0, 600.0),
        };

        let panel = match side {
            Side::Left => egui::SidePanel::left(id),
            Side::Right => egui::SidePanel::right(id),
        }
        .resizable(true)
        .default_width(default_w)
        .width_range(min_w..=max_w);

        let state = &mut self.state;
        let response = panel.show(ctx, |ui| match side {
            Side::Left => ui::draw_left_sidebar(ui, state),
            Side::Right => ui::draw_right_sidebar(ui, state),
        });

        let new_width = response.response.rect.width();
        let stored = match side {
            Side::Left => &mut state.config.left_dock_width,
            Side::Right => &mut state.config.right_dock_width,
        };
        if (new_width - *stored).abs() > 1.0 {
            *stored = new_width;
            state.config.save();
        }
    }

    fn draw_floating_sidebar(&mut self, ctx: &egui::Context, side: Side, main_rect: egui::Rect) {
        let spec = SidebarSpec::for_side(side, &self.state);
        let builder = build_floating_viewport(&spec, main_rect);

        let state = &mut self.state;
        ctx.show_viewport_immediate(
            egui::ViewportId::from_hash_of(spec.id),
            builder,
            move |viewport_ctx, _class| {
                input::handle_keyboard(state, viewport_ctx);

                egui::CentralPanel::default().show(viewport_ctx, |ui| match side {
                    Side::Left => ui::draw_left_sidebar(ui, state),
                    Side::Right => ui::draw_right_sidebar(ui, state),
                });

                if let Some(rect) = viewport_ctx.input(|i| i.viewport().inner_rect) {
                    let size = [rect.width(), rect.height()];
                    match side {
                        Side::Left => state.config.left_window_size = Some(size),
                        Side::Right => state.config.right_window_size = Some(size),
                    }
                }

                if viewport_ctx.input(|i| i.viewport().close_requested()) {
                    match side {
                        Side::Left => state.config.show_left_sidebar = false,
                        Side::Right => state.config.show_right_sidebar = false,
                    }
                    state.config.save();
                }
            },
        );
    }

    fn draw_slideshow_dialog_viewport(&mut self, ctx: &egui::Context, main_rect: egui::Rect) {
        let is_first_open = !self.state.was_slideshow_dialog_open;
        let saved_size = self.state.config.slideshow_window_size;

        let mut builder = egui::ViewportBuilder::default()
            .with_title("Slideshow Setup")
            .with_min_inner_size([320.0, 280.0]);
        if is_first_open {
            let [w, h] = saved_size.unwrap_or([520.0, 520.0]);
            // Center the dialog over the main window on first open.
            let x = main_rect.center().x - w / 2.0;
            let y = main_rect.center().y - h / 2.0;
            builder = builder.with_position([x, y]).with_inner_size([w, h]);
        }

        let state = &mut self.state;
        let thumbs = &mut self.slideshow_thumbs;
        ctx.show_viewport_immediate(
            egui::ViewportId::from_hash_of("slideshow_dialog"),
            builder,
            move |viewport_ctx, _class| {
                input::handle_keyboard(state, viewport_ctx);

                ui::draw_slideshow_dialog(viewport_ctx, state, thumbs);

                if let Some(rect) = viewport_ctx.input(|i| i.viewport().inner_rect) {
                    state.config.slideshow_window_size =
                        Some([rect.width(), rect.height()]);
                }

                if viewport_ctx.input(|i| i.viewport().close_requested()) {
                    state.slideshow_dialog.open = false;
                }
            },
        );
    }

    fn draw_search_dialog_viewport(&mut self, ctx: &egui::Context, main_rect: egui::Rect) {
        let is_first_open = !self.state.was_search_dialog_open;
        let saved_size = self.state.config.search_window_size;

        let mut builder = egui::ViewportBuilder::default()
            .with_title("Search Images")
            .with_min_inner_size([360.0, 320.0]);
        if is_first_open {
            let [w, h] = saved_size.unwrap_or([560.0, 600.0]);
            let x = main_rect.center().x - w / 2.0;
            let y = main_rect.center().y - h / 2.0;
            builder = builder.with_position([x, y]).with_inner_size([w, h]);
        }

        let state = &mut self.state;
        let thumbs = &mut self.search_thumbs;
        ctx.show_viewport_immediate(
            egui::ViewportId::from_hash_of("search_dialog"),
            builder,
            move |viewport_ctx, _class| {
                input::handle_keyboard(state, viewport_ctx);

                ui::draw_search_dialog(viewport_ctx, state, thumbs);

                if let Some(rect) = viewport_ctx.input(|i| i.viewport().inner_rect) {
                    state.config.search_window_size = Some([rect.width(), rect.height()]);
                }

                if viewport_ctx.input(|i| i.viewport().close_requested()) {
                    state.search_dialog.open = false;
                }
            },
        );
    }
}

struct SidebarSpec {
    id: &'static str,
    title: &'static str,
    is_first_open: bool,
    saved_size: Option<[f32; 2]>,
    side: Side,
}

impl SidebarSpec {
    fn for_side(side: Side, state: &AppState) -> Self {
        match side {
            Side::Left => Self {
                id: "left_sidebar",
                title: "Files",
                is_first_open: !state.was_left_sidebar_open,
                saved_size: state.config.left_window_size,
                side,
            },
            Side::Right => Self {
                id: "right_sidebar",
                title: "Tags",
                is_first_open: !state.was_right_sidebar_open,
                saved_size: state.config.right_window_size,
                side,
            },
        }
    }
}

fn build_floating_viewport(spec: &SidebarSpec, main_rect: egui::Rect) -> egui::ViewportBuilder {
    let mut builder = egui::ViewportBuilder::default()
        .with_title(spec.title)
        .with_min_inner_size([200.0, 150.0]);

    if spec.is_first_open {
        let [w, h] = spec.saved_size.unwrap_or([250.0, 500.0]);
        let (x, y) = match spec.side {
            Side::Left => (main_rect.min.x - w - 10.0, main_rect.min.y),
            Side::Right => (main_rect.max.x + 10.0, main_rect.min.y),
        };
        builder = builder.with_position([x, y]).with_inner_size([w, h]);
    }
    builder
}

fn install_japanese_font(ctx: &egui::Context) {
    let Ok(font_data) = std::fs::read("C:/Windows/Fonts/meiryo.ttc") else {
        return;
    };
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "jp_font".to_owned(),
        egui::FontData::from_owned(font_data),
    );
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        if let Some(list) = fonts.families.get_mut(&family) {
            list.insert(0, "jp_font".to_owned());
        }
    }
    ctx.set_fonts(fonts);
}
