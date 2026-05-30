use eframe::egui::{self, Color32};
use std::path::{Path, PathBuf};

use crate::thumbnail_cache::ThumbnailCache;

/// Renders `paths` as a wrap-around tile grid of thumbnails inside a vertical
/// scroll area. The column count is derived from the available width so tiles
/// fill each row left-to-right and wrap onto the next row. Rows are virtualized
/// via `show_rows`, so only the thumbnails currently on screen are decoded —
/// keeping large result sets responsive.
pub fn thumbnail_grid(
    ui: &mut egui::Ui,
    thumbs: &mut ThumbnailCache,
    paths: &[PathBuf],
    base: Option<&Path>,
    height: f32,
    thumb_dim: u32,
) {
    let total = paths.len();
    let cell = thumb_dim as f32;
    let spacing = ui.spacing().item_spacing;
    // Reserve a little width for the scrollbar so the rightmost tile never gets
    // clipped or pushed under it. Clamp to at least one column.
    let avail = (ui.available_width() - 16.0).max(cell);
    let columns = (((avail + spacing.x) / (cell + spacing.x)).floor() as usize).max(1);
    let rows = total.div_ceil(columns);
    let row_height = cell + spacing.y;

    egui::ScrollArea::vertical()
        .max_height(height)
        .auto_shrink([false, false])
        .show_rows(ui, row_height, rows, |ui, row_range| {
            for row in row_range {
                ui.horizontal(|ui| {
                    for col in 0..columns {
                        let idx = row * columns + col;
                        if idx >= total {
                            break;
                        }
                        draw_tile(ui, thumbs, &paths[idx], base, thumb_dim);
                    }
                });
            }
        });
}

fn draw_tile(
    ui: &mut egui::Ui,
    thumbs: &mut ThumbnailCache,
    path: &Path,
    base: Option<&Path>,
    thumb_dim: u32,
) {
    let max = thumb_dim as f32;
    let response = if let Some(tex) = thumbs.get(ui.ctx(), path, thumb_dim) {
        ui.add(egui::Image::new(tex).fit_to_exact_size(egui::vec2(max, max)))
    } else {
        ui.add_sized(
            egui::vec2(max, max),
            egui::Label::new(egui::RichText::new("[no preview]").color(Color32::DARK_GRAY)),
        )
    };
    // Hover shows the base-relative path (so the subfolder is visible) or the bare
    // file name when the path isn't under the base.
    let label = base
        .and_then(|b| path.strip_prefix(b).ok())
        .map(|rel| rel.display().to_string())
        .unwrap_or_else(|| {
            path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string()
        });
    response.on_hover_text(label);
}
