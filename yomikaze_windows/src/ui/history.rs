use egui::{RichText, Ui};
use crate::icons as icon;

use super::{page, widgets};
use crate::app::{Route, Shared};
use crate::model::relative_time;
use crate::theme::{self, Palette};
use crate::i18n::t;

#[derive(Default)]
pub struct HistoryState {
    confirm_clear: bool,
}

pub fn show(ui: &mut Ui, state: &mut HistoryState, shared: &mut Shared) {
    let palette = Palette::of(ui.ctx());
    let history: Vec<_> = shared.store.history().into_iter().cloned().collect();

    page(ui, "history", |ui| {
        ui.horizontal(|ui| {
            widgets::page_title(ui, t("History"));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if !history.is_empty() && widgets::icon_button(ui, icon::TRASH, 38.0, None).on_hover_text(t("Clear history")).clicked() {
                    state.confirm_clear = true;
                }
                if widgets::glass_button(ui, t("Statistics"), Some(icon::CHART_BAR), None).clicked() {
                    shared.router.push(Route::Stats);
                }
            });
        });

        if history.is_empty() {
            widgets::empty_state(ui, icon::CLOCK_COUNTER_CLOCKWISE, t("Nothing read yet"), t("Chapters you open show up here, so you can pick up where you left off."));
            return;
        }

        for item in &history {
            ui.horizontal(|ui| {
                let width = ui.available_width() - 60.0;
                let detail = format!("{} · {}", item.progress_line(), relative_time(item.last_read_at));
                let response = ui.allocate_ui(egui::vec2(width, 96.0), |ui| widgets::manga_row(ui, &mut shared.images, &item.summary(), Some(&detail))).inner;
                if response.clicked() {
                    shared.router.open(&item.summary());
                }
                response.context_menu(|ui| {
                    if ui.button(format!("{}  {}", icon::TRASH, t("Remove from history"))).clicked() {
                        shared.store.remove_from_history(&item.manga_id);
                        ui.close();
                    }
                });
                ui.vertical(|ui| {
                    ui.add_space(26.0);
                    if widgets::icon_button(ui, icon::PLAY, 44.0, Some(palette.accent)).on_hover_text(t("Continue reading")).clicked() {
                        shared.router.resume(item);
                    }
                });
            });
            ui.add_space(6.0);
        }
    });

    if state.confirm_clear {
        let mut close = false;
        let modal = egui::Modal::new(egui::Id::new("clear-history"))
            .frame(theme::modal_frame(&palette))
            .show(ui.ctx(), |ui| {
                ui.set_width(380.0);
                ui.label(RichText::new(t("Clear all reading history?")).font(theme::bold_font(20.0)));
                ui.label(RichText::new(t("Reading positions and read marks are removed. Your library and downloads stay.")).color(palette.secondary));
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    if widgets::glass_button(ui, t("Cancel"), None, Some(130.0)).clicked() {
                        close = true;
                    }
                    if widgets::primary_button(ui, t("Clear history"), Some(icon::TRASH), Some(180.0), true).clicked() {
                        let ids: Vec<String> = shared.store.history().iter().map(|m| m.manga_id.clone()).collect();
                        for id in ids {
                            shared.store.remove_from_history(&id);
                        }
                        close = true;
                    }
                });
            });
        if close || modal.should_close() {
            state.confirm_clear = false;
        }
    }
}
