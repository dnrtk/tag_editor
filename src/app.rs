use eframe::egui::{self, Color32, Key, RichText, Vec2};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::rc::Rc;

use crate::config::Config;
use crate::file_tree::{FileNode, FileTree};
use crate::image_viewer::ImageViewer;
use crate::slideshow::Slideshow;
use crate::tag_manager::{self, find_images_with_tag, is_image_file};
use image as image_crate;

pub struct TagEditorApp {
    inner: Rc<RefCell<InnerApp>>,
}

struct InnerApp {
    config: Config,
    image_viewer: ImageViewer,
    file_tree: FileTree,
    slideshow: Slideshow,

    /// 現在の画像のタグ
    current_tags: Vec<String>,
    /// タグが変更されたか
    tags_modified: bool,

    /// 新しいタグの入力
    new_tag_input: String,

    /// ホットキー設定モード
    #[allow(dead_code)]
    hotkey_config_mode: bool,
    /// 設定中のホットキー番号
    configuring_hotkey: Option<String>,
    /// ホットキータグ入力
    hotkey_tag_input: String,

    /// スライドショー設定ダイアログ
    slideshow_dialog_open: bool,
    /// スライドショー対象タグ
    slideshow_tag: String,
    /// スライドショー対象ディレクトリ
    slideshow_dir: Option<PathBuf>,

    /// ステータスメッセージ
    status_message: String,
    
    // ウィンドウ開閉状態追跡用
    was_left_sidebar_open: bool,
    was_right_sidebar_open: bool,
    /// 現在表示中のテクスチャ（spinner を出さないため同期で読み込む）
    current_texture: Option<egui::TextureHandle>,
    /// current_texture に対応する画像パス
    current_texture_path: Option<PathBuf>,
}

impl TagEditorApp {
    pub fn new(cc: &eframe::CreationContext<'_>, initial_path: Option<PathBuf>) -> Self {
        // カスタムフォントを設定（日本語対応）
        // Meiryo UI を使用（より良い日本語表示）
        let mut fonts = egui::FontDefinitions::default();
        if let Ok(font_data) = std::fs::read("C:/Windows/Fonts/meiryo.ttc") {
            fonts.font_data.insert(
                "jp_font".to_owned(),
                egui::FontData::from_owned(font_data).into(),
            );
            fonts
                .families
                .get_mut(&egui::FontFamily::Proportional)
                .unwrap()
                .insert(0, "jp_font".to_owned());
            fonts
                .families
                .get_mut(&egui::FontFamily::Monospace)
                .unwrap()
                .insert(0, "jp_font".to_owned());
        }
        cc.egui_ctx.set_fonts(fonts);

        // ダークテーマを設定
        cc.egui_ctx.set_visuals(egui::Visuals::dark());

        let mut inner = InnerApp {
            config: Config::load(),
            image_viewer: ImageViewer::default(),
            file_tree: FileTree::default(),
            slideshow: Slideshow::default(),
            current_tags: Vec::new(),
            tags_modified: false,
            new_tag_input: String::new(),
            hotkey_config_mode: false,
            configuring_hotkey: None,
            hotkey_tag_input: String::new(),
            slideshow_dialog_open: false,
            slideshow_tag: String::new(),
            slideshow_dir: None,
            status_message: String::new(),
            was_left_sidebar_open: false,
            was_right_sidebar_open: false,
            current_texture: None,
            current_texture_path: None,
        };

        // 初期パスが指定されていれば開く
        if let Some(path) = initial_path {
            if path.exists() {
                if path.is_dir() {
                    inner.file_tree.set_root(&path);
                    inner.slideshow_dir = Some(path);
                } else if is_image_file(&path) {
                    inner.open_image(path.clone());
                    if let Some(parent) = path.parent() {
                        inner.file_tree.set_root(parent);
                        inner.slideshow_dir = Some(parent.to_path_buf());
                    }
                }
            }
        }

        Self {
            inner: Rc::new(RefCell::new(inner)),
        }
    }
}

impl InnerApp {
    fn handle_dropped_files(&mut self, ctx: &egui::Context) {
        ctx.input(|i| {
            for file in &i.raw.dropped_files {
                if let Some(path) = &file.path {
                    if path.is_dir() {
                        self.file_tree.set_root(path);
                        self.slideshow_dir = Some(path.clone());
                    } else if is_image_file(path) {
                        self.open_image(path.clone());
                        if let Some(parent) = path.parent() {
                            self.file_tree.set_root(parent);
                            self.slideshow_dir = Some(parent.to_path_buf());
                        }
                    }
                }
            }
        });
    }

    fn open_image(&mut self, path: PathBuf) {
        // 変更があれば確認せずに破棄（オートセーブがオフの場合は注意）
        self.image_viewer.open(&path);
        // キャッシュされているテクスチャは新しい画像に合わせて破棄
        self.current_texture = None;
        self.current_texture_path = None;
        self.current_tags = tag_manager::load_tags(&path);
        self.tags_modified = false;
        self.status_message = format!("Opened: {}", path.display());
    }

    fn save_tags(&mut self) {
        if let Some(path) = &self.image_viewer.current_image {
            if let Err(e) = tag_manager::save_tags(path, &self.current_tags) {
                self.status_message = format!("Error saving tags: {}", e);
            } else {
                self.tags_modified = false;
                self.status_message = "Tags saved".to_string();
            }
        }
    }

    fn handle_keyboard(&mut self, ctx: &egui::Context) {
        // キー文字列変換ヘルパー
        fn key_from_str(s: &str) -> Option<Key> {
            match s.to_lowercase().as_str() {
                "0" => Some(Key::Num0), "1" => Some(Key::Num1), "2" => Some(Key::Num2),
                "3" => Some(Key::Num3), "4" => Some(Key::Num4), "5" => Some(Key::Num5),
                "6" => Some(Key::Num6), "7" => Some(Key::Num7), "8" => Some(Key::Num8),
                "9" => Some(Key::Num9),
                "a" => Some(Key::A), "b" => Some(Key::B), "c" => Some(Key::C), "d" => Some(Key::D),
                "e" => Some(Key::E), "f" => Some(Key::F), "g" => Some(Key::G), "h" => Some(Key::H),
                "i" => Some(Key::I), "j" => Some(Key::J), "k" => Some(Key::K), "l" => Some(Key::L),
                "m" => Some(Key::M), "n" => Some(Key::N), "o" => Some(Key::O), "p" => Some(Key::P),
                "q" => Some(Key::Q), "r" => Some(Key::R), "s" => Some(Key::S), "t" => Some(Key::T),
                "u" => Some(Key::U), "v" => Some(Key::V), "w" => Some(Key::W), "x" => Some(Key::X),
                "y" => Some(Key::Y), "z" => Some(Key::Z),
                _ => None,
            }
        }

        ctx.input(|i| {
            // Ctrl+S で保存
            if i.modifiers.ctrl && i.key_pressed(Key::S) {
                self.save_tags();
            }

            // Delete でゴミ箱へ
            if i.key_pressed(Key::Delete) {
                self.delete_current_image();
            }

            // 左右キーで画像移動
            if i.key_pressed(Key::ArrowLeft) && !i.modifiers.ctrl {
                self.navigate_prev();
            }
            if i.key_pressed(Key::ArrowRight) && !i.modifiers.ctrl {
                self.navigate_next();
            }

            // Ctrl+F でファイルツリー表示切り替え
            if i.modifiers.ctrl && i.key_pressed(Key::F) {
                self.config.show_left_sidebar = !self.config.show_left_sidebar;
                self.config.save();
            }

            // Ctrl+T でタグツリー表示切り替え
            if i.modifiers.ctrl && i.key_pressed(Key::T) {
                self.config.show_right_sidebar = !self.config.show_right_sidebar;
                self.config.save();
            }

            // ホットキー処理
            let hotkeys: Vec<_> = self.config.hotkey_tags.clone().into_iter().collect();
            for (key_str, tag) in hotkeys {
                if let Some(key) = key_from_str(&key_str) {
                    if i.key_pressed(key) && !i.modifiers.ctrl && !i.modifiers.alt {
                        tag_manager::toggle_tag(&mut self.current_tags, &tag);
                        self.tags_modified = true;
                        if self.config.auto_save {
                             self.save_tags();
                        }
                    }
                }
            }
        });
    }

    fn navigate_prev(&mut self) {
        if self.tags_modified && self.config.auto_save {
            self.save_tags();
        }
        self.image_viewer.prev();
        if let Some(path) = self.image_viewer.current_image.clone() {
            self.current_tags = tag_manager::load_tags(&path);
            self.tags_modified = false;
        }
    }

    fn navigate_next(&mut self) {
        if self.tags_modified && self.config.auto_save {
            self.save_tags();
        }
        self.image_viewer.next();
        if let Some(path) = self.image_viewer.current_image.clone() {
            self.current_tags = tag_manager::load_tags(&path);
            self.tags_modified = false;
        }
    }

    fn delete_current_image(&mut self) {
        if let Some(path) = self.image_viewer.current_image.clone() {
            // ゴミ箱へ移動
            if let Err(e) = trash::delete(&path) {
                self.status_message = format!("Error deleting file: {}", e);
                return;
            }

            self.status_message = format!("Moved to trash: {}", path.display());

            // リストから削除して次の画像を表示
            let mut next_path = None;
            if let Some(pos) = self.image_viewer.images_in_dir.iter().position(|p| p == &path) {
                self.image_viewer.images_in_dir.remove(pos);
                
                if !self.image_viewer.images_in_dir.is_empty() {
                    let next_idx = if pos < self.image_viewer.images_in_dir.len() {
                        pos
                    } else {
                        pos - 1
                    };
                    next_path = self.image_viewer.images_in_dir.get(next_idx).cloned();
                }
            }

            if let Some(p) = next_path {
                self.open_image(p);
            } else {
                // 画像がなくなった
                self.image_viewer.close();
                self.current_texture = None;
                self.current_texture_path = None;
            }
            
             self.current_tags.clear();
             self.tags_modified = false;
        }
    }

    fn show_left_sidebar(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            if let Some(root) = self.file_tree.root.clone() {
                self.show_file_node(ui, &root);
            } else {
                ui.label("Drop a folder or image here");
            }
        });
    }

    fn show_file_node(&mut self, ui: &mut egui::Ui, node: &FileNode) {
        if node.is_dir {
            let is_expanded = self.file_tree.is_expanded(&node.path);
            let icon = if is_expanded { "📂" } else { "📁" };

            let header = egui::CollapsingHeader::new(format!("{} {}", icon, node.name))
                .open(Some(is_expanded));

            let response = header.show(ui, |ui| {
                for child in &node.children {
                    self.show_file_node(ui, child);
                }
            });

            if response.header_response.clicked() {
                self.file_tree.toggle_expanded(&node.path);
            }
        } else {
            let is_current = self
                .image_viewer
                .current_image
                .as_ref()
                .map(|p| p == &node.path)
                .unwrap_or(false);

            let text = if is_current {
                RichText::new(format!("🖼 {}", node.name)).strong()
            } else {
                RichText::new(format!("  {}", node.name))
            };

            if ui.selectable_label(is_current, text).clicked() {
                self.open_image(node.path.clone());
            }
        }
    }

    fn show_right_sidebar(&mut self, ui: &mut egui::Ui) {
        // タグリスト
        egui::ScrollArea::vertical()
            .max_height(ui.available_height() - 100.0) // スペース調整
            .show(ui, |ui| {
                let mut tag_to_remove: Option<String> = None;

                for tag in &self.current_tags {
                    ui.horizontal(|ui| {
                        ui.label(format!("• {}", tag));
                        if ui.small_button("✕").clicked() {
                            tag_to_remove = Some(tag.clone());
                        }
                    });
                }

                if let Some(tag) = tag_to_remove {
                    tag_manager::remove_tag(&mut self.current_tags, &tag);
                    self.tags_modified = true;
                    if self.config.auto_save {
                        self.save_tags();
                    }
                }
            });

        ui.separator();

        // 新しいタグ追加
        ui.horizontal(|ui| {
            ui.label("Add:");
            let response = ui.text_edit_singleline(&mut self.new_tag_input);
            if (response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)))
                || ui.small_button("+").clicked()
            {
                if !self.new_tag_input.is_empty() {
                    tag_manager::add_tag(&mut self.current_tags, &self.new_tag_input);
                    self.new_tag_input.clear();
                    self.tags_modified = true;
                    if self.config.auto_save {
                        self.save_tags();
                    }
                }
            }
        });

        ui.separator();

        // 保存ボタン
        if ui
            .add_enabled(
                self.tags_modified,
                egui::Button::new("💾 Save (Ctrl+S)"),
            )
            .clicked()
        {
            self.save_tags();
        }

        ui.separator();

        // オートセーブ設定
        ui.checkbox(&mut self.config.auto_save, "Auto-save on hotkey");

        ui.separator();

        // ホットキー設定
        ui.collapsing("⌨ Hotkeys", |ui| {
            let mut keys: Vec<_> = self.config.hotkey_tags.keys().collect();
            keys.sort();
            
            if keys.is_empty() {
                ui.label("(No hotkeys configured)");
            }

            for key in keys {
                 if let Some(tag) = self.config.hotkey_tags.get(key) {
                     ui.horizontal(|ui| {
                        ui.label(format!("[{}]:", key));
                        ui.label(tag);
                     });
                 }
            }
            ui.separator();
            ui.label("ℹ Edit settings.json to configure hotkeys");
        });
    }

    fn show_center_panel(&mut self, ui: &mut egui::Ui) {
        if let Some(path) = &self.image_viewer.current_image {
            // テクスチャが未ロード、または別画像になっていれば同期で読み込む
            let need_load = match &self.current_texture_path {
                Some(p) => p != path,
                None => true,
            };

            if need_load {
                match image_crate::open(path) {
                    Ok(img) => {
                        let img = img.to_rgba8();
                        let (w, h) = img.dimensions();
                        let pixels = img.into_raw();
                        let color_image = egui::ColorImage::from_rgba_unmultiplied(
                            [w as usize, h as usize],
                            &pixels,
                        );
                        let ctx = ui.ctx();
                        // Texture 名にパスを使う（ユニーク）
                        let tex = ctx.load_texture(path.display().to_string(), color_image, egui::TextureOptions::default());
                        self.current_texture = Some(tex);
                        self.current_texture_path = Some(path.clone());
                    }
                    Err(_) => {
                        self.current_texture = None;
                        self.current_texture_path = None;
                    }
                }
            }

            if let Some(tex) = &self.current_texture {
                let available = ui.available_size();
                let image = egui::Image::new(tex).fit_to_exact_size(available);
                let response = ui.add(image);
                self.show_hotkey_overlay(ui, response.rect);
            } else {
                ui.centered_and_justified(|ui| {
                    ui.heading("🖼 Failed to load image");
                });
            }
        } else {
            ui.centered_and_justified(|ui| {
                ui.heading("🖼 Drop an image or folder here");
            });
        }
    }

    fn show_hotkey_overlay(&self, ui: &mut egui::Ui, rect: egui::Rect) {
        // Tag -> Vec<Key> マップ作成
        let mut tag_to_keys: HashMap<String, Vec<String>> = HashMap::new();
        for (key, tag) in &self.config.hotkey_tags {
             tag_to_keys.entry(tag.clone()).or_default().push(key.clone());
        }
        for keys in tag_to_keys.values_mut() {
            keys.sort();
        }

        // 表示対象リスト作成
        let mut tags_to_display = Vec::new();
        for tag in &self.current_tags {
            if let Some(keys) = tag_to_keys.get(tag) {
                tags_to_display.push((tag, keys));
            }
        }
        
        if tags_to_display.is_empty() {
            return;
        }

        let start_pos = rect.min + Vec2::new(10.0, 10.0);
        let mut current_pos = start_pos;
        let padding = Vec2::new(8.0, 4.0);
        let spacing = 8.0;

        let painter = ui.painter();

        for (tag, keys) in tags_to_display {
            let keys_string = keys.iter().map(|k| format!("[{}]", k)).collect::<Vec<_>>().join("");
            let text = format!("{} {}", keys_string, tag);

            let galley = painter.layout_no_wrap(
                text,
                egui::FontId::proportional(16.0),
                Color32::WHITE,
            );
            
            let rect_size = galley.size() + padding * 2.0;
            
            // 折り返し処理
            if current_pos.x + rect_size.x > rect.max.x {
                current_pos.x = start_pos.x;
                current_pos.y += rect_size.y + spacing;
            }

            let bg_rect = egui::Rect::from_min_size(current_pos, rect_size);
            
            // 枠内背景
            painter.rect_filled(
                bg_rect, 
                4.0, 
                Color32::from_rgba_unmultiplied(0, 0, 0, 180)
            );
            
            // 枠線
            painter.rect_stroke(
                bg_rect,
                4.0,
                egui::Stroke::new(1.0, Color32::from_gray(128)),
            );

            painter.galley(
                current_pos + padding,
                galley,
                Color32::WHITE,
            );

            current_pos.x += rect_size.x + spacing;
        }
    }

    fn show_menu_bar(&mut self, ui: &mut egui::Ui) {
        egui::menu::bar(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("Open Image...").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("Images", &["png", "jpg", "jpeg", "gif", "webp", "bmp"])
                        .pick_file()
                    {
                        self.open_image(path);
                    }
                    ui.close_menu();
                }
                if ui.button("Open Folder...").clicked() {
                    if let Some(path) = rfd::FileDialog::new().pick_folder() {
                        self.file_tree.set_root(&path);
                        self.slideshow_dir = Some(path);
                    }
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("Save Tags (Ctrl+S)").clicked() {
                    self.save_tags();
                    ui.close_menu();
                }
            });

            ui.menu_button("View", |ui| {
                if ui
                    .checkbox(&mut self.config.show_left_sidebar, "Files Window (Ctrl+F)")
                    .changed()
                {
                    self.config.save();
                }
                if ui
                    .checkbox(&mut self.config.show_right_sidebar, "Tags Window (Ctrl+T)")
                    .changed()
                {
                    self.config.save();
                }
            });

            ui.menu_button("Slideshow", |ui| {
                if ui.button("Start Slideshow...").clicked() {
                    self.slideshow_dialog_open = true;
                    ui.close_menu();
                }
                if self.slideshow.is_running {
                    if ui.button("Stop Slideshow").clicked() {
                        self.slideshow.stop();
                        ui.close_menu();
                    }
                }
            });

            ui.menu_button("Settings", |ui| {
                ui.horizontal(|ui| {
                    ui.label("Slideshow interval (sec):");
                    if ui
                        .add(egui::DragValue::new(&mut self.config.slideshow_interval).range(0.5..=60.0))
                        .changed()
                    {
                        self.config.save();
                    }
                });
                if ui
                    .checkbox(&mut self.config.slideshow_loop, "Loop slideshow")
                    .changed()
                {
                    self.config.save();
                }
                ui.separator();
                if ui
                    .checkbox(&mut self.config.auto_save, "Auto-save on hotkey")
                    .changed()
                {
                    self.config.save();
                }
            });
        });
    }

    fn show_slideshow_dialog(&mut self, ctx: &egui::Context) {
        let mut open = self.slideshow_dialog_open;

        egui::Window::new("Start Slideshow")
            .open(&mut open)
            .resizable(false)
            .collapsible(false)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Filter by tag:");
                    ui.text_edit_singleline(&mut self.slideshow_tag);
                });

                ui.label(format!(
                    "Directory: {}",
                    self.slideshow_dir
                        .as_ref()
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|| "(none)".to_string())
                ));

                ui.separator();

                ui.horizontal(|ui| {
                    if ui.button("Start").clicked() {
                        if let Some(dir) = &self.slideshow_dir {
                            let images = if self.slideshow_tag.is_empty() {
                                // すべての画像
                                self.image_viewer.images_in_dir.clone()
                            } else {
                                // タグでフィルタ
                                find_images_with_tag(dir, &self.slideshow_tag)
                            };

                            if !images.is_empty() {
                                self.slideshow.start(images);
                                if let Some(path) = self.slideshow.current_image().cloned() {
                                    self.open_image(path);
                                }
                                self.status_message = "Slideshow started".to_string();
                            } else {
                                self.status_message = "No images found for slideshow".to_string();
                            }
                        }
                        self.slideshow_dialog_open = false;
                    }
                    if ui.button("Cancel").clicked() {
                        self.slideshow_dialog_open = false;
                    }
                });
            });

        self.slideshow_dialog_open = open;
    }

    fn update_slideshow(&mut self) {
        if let Some(path) = self.slideshow.update(
            self.config.slideshow_interval,
            self.config.slideshow_loop,
        ) {
            self.open_image(path);
        }

        if !self.slideshow.is_running && self.slideshow.completed_once {
            self.status_message = "Slideshow completed".to_string();
            self.slideshow.completed_once = false;
        }
    }

    fn show_status_bar(&self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            // 現在の画像情報
            if let Some(path) = &self.image_viewer.current_image {
                ui.label(format!(
                    "{} ({}/{})",
                    path.file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or(""),
                    self.image_viewer.current_index + 1,
                    self.image_viewer.total_images()
                ));
            }

            ui.separator();

            // スライドショー状態
            if self.slideshow.is_running {
                ui.label(RichText::new("▶ Slideshow").color(Color32::GREEN));
            }

            // 変更状態
            if self.tags_modified {
                ui.label(RichText::new("● Modified").color(Color32::YELLOW));
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(&self.status_message);
            });
        });
    }
}

impl eframe::App for TagEditorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 1. ロジック更新とメインウィンドウ描画
        {
            let mut inner = self.inner.borrow_mut();

            // ドロップファイル処理
            inner.handle_dropped_files(ctx);

            // キーボード処理 (メインウィンドウ)
            inner.handle_keyboard(ctx);

            // スライドショー更新
            inner.update_slideshow();

            // スライドショー中は定期的に再描画
            if inner.slideshow.is_running {
                ctx.request_repaint();
            }

            // メニューバー
            egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
                inner.show_menu_bar(ui);
            });

            // ステータスバー
            egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
                inner.show_status_bar(ui);
            });

            // 中央パネル（画像表示）
            egui::CentralPanel::default().show(ctx, |ui| {
                inner.show_center_panel(ui);
            });

            // スライドショーダイアログ
            if inner.slideshow_dialog_open {
                inner.show_slideshow_dialog(ctx);
            }
        } // ここで inner の借用が解放される

        // 2. サブウィンドウの表示判定とメインウィンドウ情報の取得
        let (
            show_left,
            show_right,
            was_left_open,
            was_right_open,
            left_size_config,
            right_size_config,
            main_rect,
            screen_rect,
        ) = {
            let inner = self.inner.borrow();
            (
                inner.config.show_left_sidebar,
                inner.config.show_right_sidebar,
                inner.was_left_sidebar_open,
                inner.was_right_sidebar_open,
                inner.config.left_window_size,
                inner.config.right_window_size,
                // 現在のメインウィンドウの位置とサイズを取得
                ctx.input(|i| i.viewport().outer_rect)
                    .unwrap_or_else(|| ctx.input(|i| i.screen_rect())),
                // 画面全体の領域を取得（はみ出し防止用）
                ctx.input(|i| i.screen_rect()),
            )
        };

        // 3. 左サイドバー (OS Window)
        if show_left {
            let is_opening = !was_left_open;
            let mut builder = egui::ViewportBuilder::default()
                .with_title("Files")
                .with_min_inner_size([200.0, 150.0]);

            // 初回表示時のみ位置・サイズを設定
            if is_opening {
                // サイズ決定 (設定 or デフォルト)
                let (width, height) = if let Some(size) = left_size_config {
                    (size[0], size[1])
                } else {
                    (250.0, 500.0)
                };

                // 位置決定 (メインウィンドウの左隣)
                let x = main_rect.min.x - width - 10.0;
                let y = main_rect.min.y;

                builder = builder
                    .with_position([x, y])
                    .with_inner_size([width, height]);
            }

            let inner_shared = self.inner.clone();
            ctx.show_viewport_immediate(
                egui::ViewportId::from_hash_of("left_sidebar"),
                builder,
                move |ctx, _class| {
                    let mut inner = inner_shared.borrow_mut();
                    
                    // キーボード処理
                    inner.handle_keyboard(ctx);
                    
                    egui::CentralPanel::default().show(ctx, |ui| {
                        inner.show_left_sidebar(ui);
                    });

                    // サイズのみ保存
                    if let Some(rect) = ctx.input(|i| i.viewport().inner_rect) {
                        inner.config.left_window_size = Some([rect.width(), rect.height()]);
                    }

                    if ctx.input(|i| i.viewport().close_requested()) {
                        inner.config.show_left_sidebar = false;
                        inner.config.save();
                    }
                },
            );
        }

        // 4. 右サイドバー (OS Window)
        if show_right {
            let is_opening = !was_right_open;
            let mut builder = egui::ViewportBuilder::default()
                .with_title("Tags")
                .with_min_inner_size([200.0, 150.0]);

             // 初回表示時のみ位置・サイズを設定
            if is_opening {
                // サイズ決定 (設定 or デフォルト)
                let (width, height) = if let Some(size) = right_size_config {
                    (size[0], size[1])
                } else {
                    (250.0, 500.0)
                };

                // 位置決定 (メインウィンドウの右隣)
                let x = main_rect.max.x + 10.0;
                let y = main_rect.min.y;
                
                builder = builder
                    .with_position([x, y])
                    .with_inner_size([width, height]);
            }

            let inner_shared = self.inner.clone();
            ctx.show_viewport_immediate(
                egui::ViewportId::from_hash_of("right_sidebar"),
                builder,
                move |ctx, _class| {
                    let mut inner = inner_shared.borrow_mut();
                    
                     // キーボード処理
                    inner.handle_keyboard(ctx);

                    egui::CentralPanel::default().show(ctx, |ui| {
                        inner.show_right_sidebar(ui);
                    });
                    
                    // サイズのみ保存
                    if let Some(rect) = ctx.input(|i| i.viewport().inner_rect) {
                        inner.config.right_window_size = Some([rect.width(), rect.height()]);
                    }

                    if ctx.input(|i| i.viewport().close_requested()) {
                        inner.config.show_right_sidebar = false;
                        inner.config.save();
                    }
                },
            );
        }
        
        // 状態更新
        {
            let mut inner = self.inner.borrow_mut();
            inner.was_left_sidebar_open = show_left;
            inner.was_right_sidebar_open = show_right;
        }
    }

    fn save(&mut self, _storage: &mut dyn eframe::Storage) {
        self.inner.borrow_mut().config.save();
    }
}
