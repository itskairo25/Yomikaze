use std::collections::{HashMap, HashSet};

use chrono::{Datelike, Duration, Local, NaiveDate, TimeZone};
use egui::{Align2, CornerRadius, Rect, RichText, Sense, Ui, pos2, vec2};
use crate::icons as icon;

use super::{grid, page, widgets};
use crate::app::Shared;
use crate::model::{ChapterProgress, relative_time};
use crate::store::catalog;
use crate::theme::{self, Palette};
use crate::i18n::{t, tf};

struct Snapshot {
    chapters_read: usize,
    pages_read: usize,
    streak: usize,
    week: Vec<(String, usize)>,
    in_progress: Vec<ChapterProgress>,
    read_by_series: HashMap<String, usize>,
}

fn day_of(timestamp: i64) -> NaiveDate {
    Local.timestamp_opt(timestamp, 0).single().map(|d| d.date_naive()).unwrap_or_default()
}

fn snapshot(progress: &[ChapterProgress]) -> Snapshot {
    let completed: Vec<_> = progress.iter().filter(|p| p.completed).collect();
    let mut in_progress: Vec<ChapterProgress> = progress.iter().filter(|p| !p.completed && p.page_count > 0).cloned().collect();
    in_progress.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    let pages_read = completed.iter().map(|p| p.page_count).sum::<usize>() + in_progress.iter().map(|p| p.page + 1).sum::<usize>();

    let mut read_by_series = HashMap::new();
    for item in &completed {
        *read_by_series.entry(item.manga_id.clone()).or_insert(0) += 1;
    }

    let active: HashSet<NaiveDate> = progress.iter().map(|p| day_of(p.updated_at)).collect();
    let today = Local::now().date_naive();
    // the streak is still alive if yesterday was read but today not yet
    let mut cursor = if active.contains(&today) { today } else { today - Duration::days(1) };
    let mut streak = 0;
    while active.contains(&cursor) {
        streak += 1;
        cursor -= Duration::days(1);
    }

    let week = (0..7)
        .rev()
        .map(|offset| {
            let day = today - Duration::days(offset);
            let count = progress.iter().filter(|p| day_of(p.updated_at) == day).count();
            let label = day.weekday().to_string().chars().next().unwrap_or(' ').to_string();
            (label, count)
        })
        .collect();

    Snapshot { chapters_read: completed.len(), pages_read, streak, week, in_progress, read_by_series }
}

pub fn show(ui: &mut Ui, shared: &mut Shared) {
    let palette = Palette::of(ui.ctx());
    if ui.input(|i| i.key_pressed(egui::Key::Escape) || i.pointer.button_clicked(egui::PointerButton::Extra1)) {
        shared.router.pop();
        return;
    }
    let history: Vec<_> = shared.store.history().into_iter().cloned().collect();
    let progress: Vec<ChapterProgress> = shared.store.progress.values().cloned().collect();
    let numbers = snapshot(&progress);
    let library_count = shared.store.library().len();
    let download_count = shared.store.downloads.len();

    page(ui, "stats", |ui| {
        if widgets::glass_button(ui, t("Back"), Some(icon::CARET_LEFT), None).clicked() {
            shared.router.pop();
        }
        widgets::page_title(ui, t("Statistics"));

        if history.is_empty() {
            widgets::empty_state(ui, icon::CHART_BAR, t("No statistics yet"), t("Read a chapter and your numbers show up here."));
            return;
        }

        if let Some(last) = history.first() {
            theme::card_frame(&palette).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new(t("LAST READ")).font(theme::bold_font(11.0)).color(palette.secondary));
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    widgets::cover(ui, &mut shared.images, &last.manga_id, 84.0, 12, true);
                    ui.add_space(8.0);
                    ui.vertical(|ui| {
                        ui.label(RichText::new(&last.title).font(theme::bold_font(19.0)));
                        ui.label(RichText::new(&last.last_chapter_name).color(palette.secondary));
                        if last.last_page_count > 0 {
                            let fraction = (last.last_page + 1) as f32 / last.last_page_count as f32;
                            ui.set_max_width(420.0);
                            widgets::progress_bar(ui, fraction, 6.0);
                            ui.label(
                                RichText::new(tf("Page {page} of {count} · {percent}%", &[("page", &(last.last_page + 1)), ("count", &last.last_page_count), ("percent", &(fraction * 100.0).round())]))
                                    .size(12.5)
                                    .color(palette.secondary),
                            );
                        }
                        ui.label(RichText::new(relative_time(last.last_read_at)).size(12.0).color(palette.tertiary));
                        ui.add_space(6.0);
                        if widgets::primary_button(ui, t("Continue reading"), Some(icon::PLAY), Some(220.0), true).clicked() {
                            shared.router.resume(last);
                        }
                    });
                });
            });
            ui.add_space(18.0);
        }

        let tiles = [
            (numbers.chapters_read.to_string(), t("Chapters read"), icon::CHECK_CIRCLE),
            (numbers.pages_read.to_string(), t("Pages read"), icon::FILE_TEXT),
            (history.len().to_string(), t("Series started"), icon::BOOK),
            (numbers.streak.to_string(), if numbers.streak == 1 { t("Day streak") } else { t("Days streak") }, icon::FIRE),
            (library_count.to_string(), t("In library"), icon::BOOKMARK_SIMPLE),
            (download_count.to_string(), t("Downloaded chapters"), icon::DOWNLOAD_SIMPLE),
        ];
        let tile_min = if ui.available_width() >= 6.0 * 160.0 + 5.0 * 12.0 { 160.0 } else { (ui.available_width() - 2.0 * 12.0) / 3.0 - 1.0 };
        grid(ui, tiles.len(), tile_min, 12.0, |ui, index, width| {
            let (value, label, glyph) = &tiles[index];
            let (rect, _) = ui.allocate_exact_size(vec2(width, 118.0), Sense::hover());
            ui.painter().rect_filled(rect, CornerRadius::same(22), palette.glass_strong.gamma_multiply(0.75));
            ui.painter().text(rect.min + vec2(18.0, 18.0), Align2::LEFT_TOP, *glyph, theme::icon_font(22.0), palette.accent);
            ui.painter().text(rect.min + vec2(18.0, 46.0), Align2::LEFT_TOP, value, theme::heavy_font(32.0), palette.text);
            ui.painter().text(rect.min + vec2(18.0, 88.0), Align2::LEFT_TOP, *label, theme::font(13.0), palette.secondary);
        });
        ui.add_space(8.0);

        ui.label(RichText::new(t("Last 7 days")).font(theme::bold_font(21.0)));
        ui.add_space(6.0);
        let peak = numbers.week.iter().map(|d| d.1).max().unwrap_or(0).max(1);
        let width = ui.available_width();
        let (rect, _) = ui.allocate_exact_size(vec2(width, 190.0), Sense::hover());
        ui.painter().rect_filled(rect, CornerRadius::same(24), palette.glass_strong.gamma_multiply(0.75));
        let inner = rect.shrink2(vec2(24.0, 20.0));
        let column = inner.width() / 7.0;
        for (index, (label, count)) in numbers.week.iter().enumerate() {
            let x = inner.left() + column * (index as f32 + 0.5);
            let height = (110.0 * *count as f32 / peak as f32).max(6.0);
            let bar = Rect::from_min_max(pos2(x - column * 0.28, inner.bottom() - 22.0 - height), pos2(x + column * 0.28, inner.bottom() - 22.0));
            ui.painter().rect_filled(bar, CornerRadius::same(7), if *count > 0 { palette.accent } else { palette.quaternary });
            if *count > 0 {
                ui.painter().text(pos2(x, bar.top() - 6.0), Align2::CENTER_BOTTOM, count.to_string(), theme::bold_font(12.0), palette.secondary);
            }
            ui.painter().text(pos2(x, inner.bottom()), Align2::CENTER_BOTTOM, label, theme::font(12.0), palette.secondary);
        }
        ui.add_space(18.0);

        let titles: HashMap<String, String> = history.iter().map(|m| (m.manga_id.clone(), m.title.clone())).collect();

        if !numbers.in_progress.is_empty() {
            ui.label(RichText::new(t("Chapters in progress")).font(theme::bold_font(21.0)));
            ui.add_space(6.0);
            theme::card_frame(&palette).show(ui, |ui| {
                ui.set_width(ui.available_width());
                for item in numbers.in_progress.iter().take(8) {
                    let fraction = (item.page + 1) as f32 / item.page_count.max(1) as f32;
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.label(RichText::new(&item.chapter_name).font(theme::bold_font(14.5)));
                            ui.label(RichText::new(titles.get(&item.manga_id).cloned().unwrap_or_default()).size(12.5).color(palette.secondary));
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(RichText::new(format!("{}%", (fraction * 100.0).round())).font(theme::bold_font(15.0)).color(palette.accent));
                        });
                    });
                    widgets::progress_bar(ui, fraction, 5.0);
                    ui.add_space(10.0);
                }
            });
            ui.add_space(18.0);
        }

        ui.label(RichText::new(t("Progress by series")).font(theme::bold_font(21.0)));
        ui.add_space(6.0);
        theme::card_frame(&palette).show(ui, |ui| {
            ui.set_width(ui.available_width());
            for manga in &history {
                let read = numbers.read_by_series.get(&manga.manga_id).copied().unwrap_or(0);
                let total = catalog::chapters(&manga.manga_id).len().max(read);
                let fraction = if total > 0 { read as f32 / total as f32 } else { 0.0 };
                let response = ui
                    .horizontal(|ui| {
                        widgets::cover(ui, &mut shared.images, &manga.manga_id, 46.0, 8, true);
                        ui.vertical(|ui| {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(&manga.title).font(theme::bold_font(14.5)));
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    ui.label(RichText::new(format!("{}%", (fraction * 100.0).round())).font(theme::bold_font(15.0)).color(palette.accent));
                                });
                            });
                            widgets::progress_bar(ui, fraction, 5.0);
                            let line = if total > 0 {
                                tf("{read} of {total} chapters · last: {chapter}", &[("read", &read), ("total", &total), ("chapter", &manga.last_chapter_name)])
                            } else {
                                tf("Last: {chapter}", &[("chapter", &manga.last_chapter_name)])
                            };
                            ui.label(RichText::new(line).size(12.5).color(palette.secondary));
                        });
                    })
                    .response
                    .interact(Sense::click());
                if response.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                    shared.router.open(&manga.summary());
                }
                ui.add_space(10.0);
            }
        });
    });
}
