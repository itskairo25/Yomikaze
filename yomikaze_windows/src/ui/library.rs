use egui::{RichText, Ui};
use crate::icons as icon;

use super::{grid, page, widgets};
use crate::app::{LibrarySection, Shared, Tab};
use crate::model::MangaSummary;
use crate::net;
use crate::theme::{self, Palette};
use crate::i18n::{t, tf};

#[derive(Default)]
pub struct LibraryState {
    // empty means all
    selected_category: String,
    new_category: Option<String>,
}

struct DownloadGroup {
    id: String,
    title: String,
    count: usize,
}

pub fn show(ui: &mut Ui, state: &mut LibraryState, shared: &mut Shared) {
    let palette = Palette::of(ui.ctx());
    page(ui, "library", |ui| {
        ui.horizontal(|ui| {
            widgets::page_title(ui, t("Library"));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if shared.router.library_section == LibrarySection::Saved {
                    sort_menu(ui, shared);
                }
            });
        });

        ui.horizontal(|ui| {
            for (section, label, glyph) in [
                (LibrarySection::Saved, t("Saved"), icon::BOOKMARK_SIMPLE),
                (LibrarySection::Downloads, t("Downloads"), icon::DOWNLOAD_SIMPLE),
            ] {
                if widgets::pill(ui, label, Some(glyph), shared.router.library_section == section, 150.0).clicked() {
                    shared.router.library_section = section;
                }
            }
        });
        ui.add_space(14.0);

        if !net::is_online() {
            widgets::offline_banner(ui, t("You're offline. Downloaded chapters are still readable."));
            ui.add_space(14.0);
        }

        match shared.router.library_section {
            LibrarySection::Saved => saved(ui, state, shared, &palette),
            LibrarySection::Downloads => downloads(ui, shared, &palette),
        }
    });
}

fn sort_menu(ui: &mut Ui, shared: &mut Shared) {
    let current = shared.store.settings.library_sort.clone();
    ui.menu_button(RichText::new(format!("{}  {}", icon::SORT_ASCENDING, t("Sort"))).size(14.5), |ui| {
        for (key, label) in [("recentlyAdded", t("Recently added")), ("lastRead", t("Last read")), ("title", t("Title"))] {
            if ui.selectable_label(current == key, label).clicked() {
                shared.store.settings.library_sort = key.to_string();
                shared.store.settings_changed();
                ui.close();
            }
        }
    });
}

fn saved(ui: &mut Ui, state: &mut LibraryState, shared: &mut Shared, palette: &Palette) {
    let library: Vec<_> = shared.store.library().into_iter().cloned().collect();
    if library.is_empty() {
        widgets::empty_state(
            ui,
            icon::BOOKS,
            t("Your library is empty"),
            t("Click the bookmark on any series to keep it here. Everything is stored on this PC."),
        );
        ui.vertical_centered(|ui| {
            if widgets::primary_button(ui, t("Browse manga"), Some(icon::COMPASS), None, true).clicked() {
                shared.router.select(Tab::Browse);
            }
        });
        return;
    }

    category_bar(ui, state, shared, &library, palette);
    ui.add_space(16.0);

    let items: Vec<_> = library
        .iter()
        .filter(|m| state.selected_category.is_empty() || m.category == state.selected_category)
        .cloned()
        .collect();
    if items.is_empty() {
        widgets::empty_state(
            ui,
            icon::FOLDER,
            &tf("Nothing in {category}", &[("category", &state.selected_category)]),
            t("Right-click a series and choose Move to category."),
        );
        return;
    }

    let categories = shared.store.settings.categories.clone();
    grid(ui, items.len(), 150.0, 18.0, |ui, index, width| {
        let item = &items[index];
        let subtitle = if item.has_history { item.last_chapter_name.clone() } else { t("Not started").to_string() };
        let response = widgets::manga_card(ui, &mut shared.images, &item.summary(), Some(&subtitle), width);
        if response.clicked() {
            shared.router.open(&item.summary());
        }
        response.context_menu(|ui| {
            if item.has_history && ui.button(format!("{}  {}", icon::PLAY, t("Continue reading"))).clicked() {
                shared.router.resume(item);
                ui.close();
            }
            ui.menu_button(format!("{}  {}", icon::FOLDER, t("Move to category")), |ui| {
                if ui.selectable_label(item.category.is_empty(), t("None")).clicked() {
                    if let Some(m) = shared.store.manga.get_mut(&item.manga_id) {
                        m.category.clear();
                    }
                    shared.store.mark_dirty();
                    ui.close();
                }
                for name in &categories {
                    if ui.selectable_label(&item.category == name, name).clicked() {
                        if let Some(m) = shared.store.manga.get_mut(&item.manga_id) {
                            m.category = name.clone();
                        }
                        shared.store.mark_dirty();
                        ui.close();
                    }
                }
                if categories.is_empty() {
                    ui.label(RichText::new(t("Create a category with + first.")).size(12.5));
                }
            });
            ui.separator();
            if ui.button(RichText::new(format!("{}  {}", icon::BOOKMARK_SIMPLE, t("Remove from library"))).color(egui::Color32::from_rgb(255, 99, 99))).clicked() {
                shared.store.remove_from_library(&item.manga_id);
                ui.close();
            }
        });
    });
}

fn category_bar(ui: &mut Ui, state: &mut LibraryState, shared: &mut Shared, library: &[crate::model::SavedManga], palette: &Palette) {
    let categories = shared.store.settings.categories.clone();
    ui.horizontal_wrapped(|ui| {
        let label = format!("{}  {}", t("All"), library.len());
        if widgets::pill(ui, &label, Some(icon::SQUARES_FOUR), state.selected_category.is_empty(), 0.0).clicked() {
            state.selected_category.clear();
        }
        for name in &categories {
            let count = library.iter().filter(|m| &m.category == name).count();
            let response = widgets::pill(ui, &format!("{name}  {count}"), Some(icon::FOLDER), &state.selected_category == name, 0.0);
            if response.clicked() {
                state.selected_category = name.clone();
            }
            response.context_menu(|ui| {
                if ui.button(format!("{}  {}", icon::TRASH, t("Delete category"))).clicked() {
                    for m in shared.store.manga.values_mut() {
                        if &m.category == name {
                            m.category.clear();
                        }
                    }
                    shared.store.settings.categories.retain(|c| c != name);
                    shared.store.settings_changed();
                    shared.store.mark_dirty();
                    if &state.selected_category == name {
                        state.selected_category.clear();
                    }
                    ui.close();
                }
            });
        }
        if widgets::icon_button(ui, icon::PLUS, 38.0, None).on_hover_text(t("New category")).clicked() {
            state.new_category = Some(String::new());
        }
    });

    let mut close = false;
    if let Some(name) = &mut state.new_category {
        let modal = egui::Modal::new(egui::Id::new("new-category"))
            .frame(theme::modal_frame(palette))
            .show(ui.ctx(), |ui| {
                ui.set_width(340.0);
                ui.label(RichText::new(t("New category")).font(theme::bold_font(20.0)));
                ui.label(RichText::new(t("For example Reading, Finished or Later.")).color(palette.secondary));
                ui.add_space(8.0);
                let edit = ui.add(egui::TextEdit::singleline(name).hint_text(t("Name")).desired_width(f32::INFINITY));
                edit.request_focus();
                ui.add_space(12.0);
                let mut add = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                ui.horizontal(|ui| {
                    if widgets::glass_button(ui, t("Cancel"), None, Some(120.0)).clicked() {
                        close = true;
                    }
                    if widgets::primary_button(ui, t("Add"), None, Some(120.0), !name.trim().is_empty()).clicked() {
                        add = true;
                    }
                });
                add
            });
        if modal.inner {
            let trimmed = name.trim().to_string();
            let categories = &mut shared.store.settings.categories;
            if !trimmed.is_empty() && !categories.iter().any(|c| c.eq_ignore_ascii_case(&trimmed)) {
                categories.push(trimmed.clone());
                shared.store.settings_changed();
                state.selected_category = trimmed;
            }
            close = true;
        }
        if modal.should_close() {
            close = true;
        }
    }
    if close {
        state.new_category = None;
    }
}

fn downloads(ui: &mut Ui, shared: &mut Shared, palette: &Palette) {
    let active = shared.downloads.active_count();
    if active > 0 {
        ui.label(RichText::new(format!("{}  {}", icon::DOWNLOAD_SIMPLE, tf("Downloading {n} chapters...", &[("n", &active)]))).color(palette.secondary));
        ui.add_space(10.0);
    }

    let mut list: Vec<_> = shared.store.downloads.values().collect();
    list.sort_by(|a, b| b.downloaded_at.cmp(&a.downloaded_at));
    let mut groups: Vec<DownloadGroup> = Vec::new();
    for item in list {
        if let Some(group) = groups.iter_mut().find(|g| g.id == item.manga_id) {
            group.count += 1;
        } else {
            groups.push(DownloadGroup { id: item.manga_id.clone(), title: item.manga_title.clone(), count: 1 });
        }
    }

    if groups.is_empty() {
        widgets::empty_state(
            ui,
            icon::DOWNLOAD_SIMPLE,
            t("No downloads yet"),
            t("Download chapters from a series page to read them without a connection."),
        );
        return;
    }

    for group in groups {
        let detail = if group.count == 1 { t("1 chapter downloaded").to_string() } else { tf("{n} chapters downloaded", &[("n", &group.count)]) };
        let manga = MangaSummary::new(group.id.clone(), group.title.clone());
        let response = widgets::manga_row(ui, &mut shared.images, &manga, Some(&detail));
        if response.clicked() {
            shared.router.open(&manga);
        }
        response.context_menu(|ui| {
            if ui.button(format!("{}  {}", icon::TRASH, t("Delete downloads"))).clicked() {
                delete_series_downloads(shared, &group.id);
                ui.close();
            }
        });
        ui.add_space(6.0);
    }
}

pub fn delete_series_downloads(shared: &mut Shared, manga_id: &str) {
    shared.downloads.cancel_series(manga_id);
    let urls: Vec<String> = shared
        .store
        .downloads
        .values()
        .filter(|d| d.manga_id == manga_id)
        .flat_map(|d| d.page_urls.clone())
        .collect();
    shared.store.downloads.retain(|_, d| d.manga_id != manga_id);
    shared.store.mark_dirty();
    std::thread::spawn(move || crate::images::disk::remove_downloads(&urls));
}
