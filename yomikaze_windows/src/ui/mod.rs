pub mod browse;
pub mod detail;
pub mod history;
pub mod library;
pub mod reader;
pub mod search;
pub mod settings;
pub mod sharecard;
pub mod stats;
pub mod widgets;

use egui::{Ui, vec2};

// scrolling page with margins and a readable max width
pub fn page<R>(ui: &mut Ui, id: &str, add: impl FnOnce(&mut Ui) -> R) -> R {
    let mut result = None;
    egui::ScrollArea::vertical()
        .id_salt(id)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let full = ui.available_width();
            let width = (full - 72.0).clamp(300.0, 1480.0);
            let margin = ((full - width) / 2.0).max(24.0);
            ui.horizontal(|ui| {
                ui.add_space(margin);
                ui.vertical(|ui| {
                    ui.set_width(width);
                    ui.add_space(26.0);
                    result = Some(add(ui));
                    ui.add_space(40.0);
                });
            });
        });
    result.expect("page content ran")
}

pub fn grid(ui: &mut Ui, count: usize, min_width: f32, spacing: f32, mut cell: impl FnMut(&mut Ui, usize, f32)) {
    let (columns, width) = widgets::grid_columns(ui.available_width(), min_width, spacing);
    let mut index = 0;
    while index < count {
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = spacing;
            for _ in 0..columns {
                if index >= count {
                    break;
                }
                ui.allocate_ui(vec2(width, 0.0), |ui| {
                    ui.set_width(width);
                    cell(ui, index, width);
                });
                index += 1;
            }
        });
        ui.add_space(spacing + 4.0);
    }
}
