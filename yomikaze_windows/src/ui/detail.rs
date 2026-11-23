use std::collections::HashMap;

use egui::{Align2, Color32, CornerRadius, Rect, RichText, Sense, Stroke, StrokeKind, Ui, pos2, vec2};
use crate::icons as icon;
use poll_promise::Promise;

use super::{page, sharecard, widgets};
use crate::app::{Shared, spawn};
use crate::images::{Kind, State};
use crate::model::{Artwork, Chapter, ChapterProgress, MangaDetail, MangaSummary, short_date};
use crate::net;
use crate::source;
use crate::store::catalog;
use crate::theme::{self, Palette};
use crate::i18n::{t, tf};

pub struct DetailState {
    manga: MangaSummary,
    detail: Option<MangaDetail>,
    chapters: Vec<Chapter>,
    artwork: Option<Artwork>,
    loading: Option<Promise<(source::Result<MangaDetail>, source::Result<Vec<Chapter>>)>>,
    artwork_request: Option<Promise<source::Result<Artwork>>>,
    error: Option<String>,
    loaded_once: bool,
    summary_expanded: bool,
    oldest_first: bool,
    downloaded_only: bool,
    confirm_download: Option<Vec<Chapter>>,
    confirm_delete: bool,
    share: sharecard::ShareState,
}

impl DetailState {
    pub fn new(manga: MangaSummary) -> Self {
        let id = manga.id.clone();
        Self {
            detail: catalog::load(&catalog::detail_name(&id)),
            chapters: catalog::chapters(&id),
            artwork: catalog::load(&catalog::artwork_name(&id)),
            manga,
            loading: None,
            artwork_request: None,
            error: None,
            loaded_once: false,
            summary_expanded: false,
            oldest_first: false,
            downloaded_only: false,
            confirm_download: None,
            confirm_delete: false,
            share: Default::default(),
        }
    }

    fn title(&self) -> String {
        self.detail.as_ref().map(|d| d.title.clone()).unwrap_or_else(|| self.manga.title.clone())
    }

    fn status(&self) -> String {
        self.detail.as_ref().map(|d| d.status.clone()).or_else(|| self.manga.status.clone()).unwrap_or_default()
    }

    fn kind(&self) -> String {
        self.detail.as_ref().map(|d| d.kind.clone()).or_else(|| self.manga.kind.clone()).unwrap_or_default()
    }

    fn year(&self) -> String {
        self.detail.as_ref().map(|d| d.released.clone()).or_else(|| self.manga.year.clone()).unwrap_or_default()
    }

    fn authors(&self) -> String {
        self.detail.as_ref().map(|d| d.authors.join(", ")).unwrap_or_default()
    }

    fn reload(&mut self, ui: &Ui) {
        if self.loading.is_some() || !net::is_online() {
            return;
        }
        let id = self.manga.id.clone();
        self.loading = Some(spawn(ui.ctx(), move || {
            let chapters = source::chapters(&id);
            let detail = source::detail(&id);
            (detail, chapters)
        }));
    }

    fn poll(&mut self, ui: &Ui, shared: &mut Shared) {
        if let Some(promise) = &self.loading {
            if let Some((detail, chapters)) = promise.ready() {
                let id = self.manga.id.clone();
                match chapters {
                    Ok(list) if !list.is_empty() => {
                        self.chapters = list.clone();
                        catalog::save(&catalog::chapters_name(&id), list);
                        self.error = None;
                        apply_anilist_progress(&id, list, shared);
                    }
                    Ok(_) => {}
                    Err(error) => {
                        if self.chapters.is_empty() {
                            self.error = Some(error.clone());
                        }
                    }
                }
                if let Ok(detail) = detail {
                    self.detail = Some(detail.clone());
                    catalog::save(&catalog::detail_name(&id), detail);
                    if let Some(saved) = shared.store.manga.get_mut(&id) {
                        saved.title = detail.title.clone();
                        saved.author = detail.authors.join(", ");
                        saved.status = detail.status.clone();
                        saved.kind = detail.kind.clone();
                        shared.store.mark_dirty();
                    }
                }
                self.loading = None;
                self.loaded_once = true;

                // artwork is optional, look it up last and once per series
                if self.artwork.is_none() && self.artwork_request.is_none() {
                    let anilist = self.detail.as_ref().and_then(|d| d.anilist_id);
                    let title = self.title();
                    self.artwork_request = Some(spawn(ui.ctx(), move || source::artwork(anilist, &title)));
                }
            }
        }
        if let Some(promise) = &self.artwork_request {
            if let Some(result) = promise.ready() {
                if let Ok(art) = result {
                    catalog::save(&catalog::artwork_name(&self.manga.id), art);
                    self.artwork = Some(art.clone());
                }
                self.artwork_request = None;
            }
        }
    }

    // in progress, next unread, or the first
    fn resume_target(&self, shared: &Shared) -> Option<Chapter> {
        if self.chapters.is_empty() {
            return None;
        }
        if let Some(saved) = shared.store.saved(&self.manga.id) {
            if let Some(index) = self.chapters.iter().position(|c| c.id == saved.last_chapter_id) {
                let finished = shared.store.progress.get(&saved.last_chapter_id).map(|p| p.completed).unwrap_or(false);
                // chapters are newest first, so the next one is one index lower
                if finished && index > 0 {
                    return Some(self.chapters[index - 1].clone());
                }
                return Some(self.chapters[index].clone());
            }
        }
        self.chapters.last().cloned()
    }
}

pub fn show(ui: &mut Ui, state: &mut DetailState, shared: &mut Shared) {
    let palette = Palette::of(ui.ctx());
    if !state.loaded_once && state.loading.is_none() {
        state.reload(ui);
        if !net::is_online() {
            state.loaded_once = true;
            if shared.store.downloads.values().any(|d| d.manga_id == state.manga.id) {
                state.downloaded_only = true;
            }
        }
    }
    state.poll(ui, shared);

    // back with escape or the mouse back button
    if ui.input(|i| i.key_pressed(egui::Key::Escape) || i.pointer.button_clicked(egui::PointerButton::Extra1)) {
        shared.router.pop();
        return;
    }

    let progress: HashMap<String, ChapterProgress> = shared
        .store
        .progress
        .values()
        .filter(|p| p.manga_id == state.manga.id)
        .map(|p| (p.chapter_id.clone(), p.clone()))
        .collect();
    let downloaded: std::collections::HashSet<String> = shared
        .store
        .downloads
        .values()
        .filter(|d| d.manga_id == state.manga.id)
        .map(|d| d.chapter_id.clone())
        .collect();

    // blurred cover and the anilist banner once loaded, filling the content area next to the sidebar
    let area = ui.clip_rect();
    let banner_url = state.artwork.as_ref().and_then(|a| a.banner.clone());
    let banner_ready = banner_url
        .as_ref()
        .map(|url| matches!(shared.images.get(url, Kind::Wide), State::Ready(_)))
        .unwrap_or(false);
    {
        let painter = ui.ctx().layer_painter(ui.layer_id()).with_clip_rect(area);
        if shared.store.settings.wallpaper_rev == 0 {
            let glow = Rect::from_min_size(area.min, vec2(area.width(), 620.0));
            let url = crate::source::cover_url(&state.manga.id, true);
            if let State::Ready(loaded) = shared.images.get(&url, Kind::Blur) {
                paint_faded(&painter, loaded.first(), loaded.tiles[0].1, glow, if palette.dark { 0.5 } else { 0.28 }, None);
            }
        }
        if let (true, Some(url)) = (banner_ready, &banner_url) {
            let banner = Rect::from_min_size(area.min, vec2(area.width(), 320.0));
            if let State::Ready(loaded) = shared.images.get(url, Kind::Wide) {
                paint_faded(&painter, loaded.first(), loaded.tiles[0].1, banner, 1.0, Some(if palette.dark { Color32::from_black_alpha(100) } else { palette.background.gamma_multiply(0.7) }));
            }
        }
    }

    page(ui, &format!("detail-{}", state.manga.id), |ui| {
        top_bar(ui, state, shared, &palette);
        let desktop = ui.available_width() >= 760.0;
        if desktop {
            ui.add_space(if banner_ready { 92.0 } else { 10.0 });
            header_wide(ui, state, shared, &palette);
        } else {
            ui.add_space(if banner_ready { 120.0 } else { 6.0 });
            header(ui, state, shared, &palette);
            ui.add_space(14.0);
            fact_strip(ui, state, &palette);
        }
        ui.add_space(22.0);

        if !net::is_online() {
            widgets::offline_banner(ui, t("You're offline. Downloaded chapters are still readable."));
            ui.add_space(14.0);
        }

        ui.horizontal_top(|ui| {
            let wide = ui.available_width() > 980.0;
            if wide {
                // two columns on wide windows
                let left_width = 390.0;
                ui.vertical(|ui| {
                    ui.set_width(left_width);
                    progress_card(ui, state, shared, &progress, &palette);
                    ui.add_space(14.0);
                    about_card(ui, state, &palette);
                });
                ui.add_space(22.0);
                ui.vertical(|ui| {
                    chapters(ui, state, shared, &progress, &downloaded, &palette);
                });
            } else {
                ui.vertical(|ui| {
                    progress_card(ui, state, shared, &progress, &palette);
                    ui.add_space(14.0);
                    about_card(ui, state, &palette);
                    ui.add_space(26.0);
                    chapters(ui, state, shared, &progress, &downloaded, &palette);
                });
            }
        });
    });

    dialogs(ui, state, shared, &palette);
    sharecard::show_modal(ui, &mut state.share, shared, &palette);
}

// fades to transparent at the bottom, scrim tints the middle so the title stays readable
fn paint_faded(painter: &egui::Painter, texture: &egui::TextureHandle, size: egui::Vec2, rect: Rect, strength: f32, scrim: Option<Color32>) {
    let target = rect.width() / rect.height();
    let source = size.x / size.y.max(1.0);
    let (u0, u1, v0, v1) = if source > target {
        let span = target / source;
        (0.5 - span / 2.0, 0.5 + span / 2.0, 0.0, 1.0)
    } else {
        let span = source / target;
        // keep more of the top, where faces usually are
        (0.0, 1.0, (0.35 - span / 2.0).max(0.0), (0.35 - span / 2.0).max(0.0) + span)
    };
    let mut mesh = egui::Mesh::with_texture(texture.id());
    let rows = [(0.0, 1.0), (0.5, 1.0), (0.8, 0.45), (1.0, 0.0)];
    for (t, alpha) in rows {
        let y = rect.top() + rect.height() * t;
        let v = v0 + (v1 - v0) * t;
        let color = Color32::from_white_alpha((255.0 * alpha * strength) as u8);
        mesh.vertices.push(egui::epaint::Vertex { pos: pos2(rect.left(), y), uv: pos2(u0, v), color });
        mesh.vertices.push(egui::epaint::Vertex { pos: pos2(rect.right(), y), uv: pos2(u1, v), color });
    }
    for row in 0..rows.len() as u32 - 1 {
        let i = row * 2;
        mesh.add_triangle(i, i + 1, i + 2);
        mesh.add_triangle(i + 1, i + 3, i + 2);
    }
    painter.add(mesh);
    if let Some(shade) = scrim {
        let y = |t: f32| rect.top() + rect.height() * t;
        theme::vertical_gradient(painter, Rect::from_x_y_ranges(rect.x_range(), y(0.35)..=y(0.75)), Color32::TRANSPARENT, shade);
        theme::vertical_gradient(painter, Rect::from_x_y_ranges(rect.x_range(), y(0.75)..=y(1.0)), shade, Color32::TRANSPARENT);
    }
}

fn top_bar(ui: &mut Ui, state: &mut DetailState, shared: &mut Shared, palette: &Palette) {
    ui.horizontal(|ui| {
        if widgets::glass_button(ui, t("Back"), Some(icon::CARET_LEFT), None).clicked() {
            shared.router.pop();
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let more = widgets::icon_button(ui, icon::DOTS_THREE, 40.0, None).on_hover_text(t("More"));
            egui::Popup::menu(&more).show(|ui| {
                ui.set_min_width(240.0);
                ui.label(RichText::new(t("Downloads")).size(12.0).color(palette.secondary));
                if ui.button(format!("{}  {}", icon::DOWNLOAD_SIMPLE, t("Download all chapters"))).clicked() {
                    request_download(state, shared, false);
                    ui.close();
                }
                if ui.button(format!("{}  {}", icon::DOWNLOAD, t("Download unread chapters"))).clicked() {
                    request_download(state, shared, true);
                    ui.close();
                }
                if shared.store.downloads.values().any(|d| d.manga_id == state.manga.id)
                    && ui.button(format!("{}  {}", icon::TRASH, t("Delete downloads"))).clicked()
                {
                    state.confirm_delete = true;
                    ui.close();
                }
                ui.separator();
                ui.label(RichText::new(t("Reading")).size(12.0).color(palette.secondary));
                if ui.button(format!("{}  {}", icon::CHECK_CIRCLE, t("Mark all as read"))).clicked() {
                    for chapter in state.chapters.clone() {
                        shared.store.set_read(&state.manga.id, &chapter, true);
                    }
                    ui.close();
                }
                if ui.button(format!("{}  {}", icon::CIRCLE, t("Mark all as unread"))).clicked() {
                    for chapter in state.chapters.clone() {
                        shared.store.set_read(&state.manga.id, &chapter, false);
                    }
                    ui.close();
                }
                ui.separator();
                if ui.button(format!("{}  {}", icon::IMAGE, t("Share as card"))).clicked() {
                    let accent = palette.accent;
                    let info = sharecard::CardInfo {
                        manga_id: state.manga.id.clone(),
                        title: state.title(),
                        authors: state.authors(),
                        chapter_count: state.chapters.len(),
                        status: state.status(),
                        kind: state.kind(),
                        year: state.year(),
                        tags: state.detail.as_ref().map(|d| d.tags.clone()).unwrap_or_default(),
                        accent: [accent.r(), accent.g(), accent.b()],
                    };
                    state.share.start(ui.ctx(), info);
                    ui.close();
                }
                if ui.button(format!("{}  {}", icon::LINK, t("Copy link"))).clicked() {
                    ui.ctx().copy_text(source::series_url(&state.manga.id));
                    shared.router.toast(t("Link copied"));
                    ui.close();
                }
                if ui.button(format!("{}  {}", icon::GLOBE, t("Open in browser"))).clicked() {
                    let _ = open::that(source::series_url(&state.manga.id));
                    ui.close();
                }
            });

            if widgets::icon_button(ui, icon::ARROWS_CLOCKWISE, 36.0, None).on_hover_text(t("Refresh")).clicked() {
                state.loaded_once = false;
            }
        });
    });
}

fn header(ui: &mut Ui, state: &DetailState, shared: &mut Shared, palette: &Palette) {
    ui.vertical_centered(|ui| {
        let width = 200.0;
        let (rect, _) = ui.allocate_exact_size(vec2(width, width * 1.5), Sense::hover());
        for (offset, alpha) in [(18.0, 40), (9.0, 50)] {
            ui.painter().rect_filled(rect.translate(vec2(0.0, offset)).shrink(6.0), CornerRadius::same(22), Color32::from_black_alpha(alpha));
        }
        widgets::paint_cover(ui, &mut shared.images, &state.manga.id, rect, 22, false);
        ui.add_space(14.0);
        ui.add(egui::Label::new(RichText::new(state.title()).font(theme::heavy_font(32.0))).wrap());
        let authors = state.authors();
        if !authors.is_empty() {
            ui.label(RichText::new(authors).size(15.0).color(palette.secondary));
        }
        if state.detail.as_ref().map(|d| d.official).unwrap_or(false) {
            ui.label(RichText::new(format!("{}  {}", icon::SEAL_CHECK, t("Official translation"))).size(13.0).color(palette.accent));
        }
    });
}

fn header_wide(ui: &mut Ui, state: &DetailState, shared: &mut Shared, palette: &Palette) {
    ui.horizontal_top(|ui| {
        let width = 176.0;
        let (rect, _) = ui.allocate_exact_size(vec2(width, width * 1.5), Sense::hover());
        ui.painter().rect_filled(rect.translate(vec2(0.0, 12.0)).shrink(8.0), CornerRadius::same(20), Color32::from_black_alpha(60));
        widgets::paint_cover(ui, &mut shared.images, &state.manga.id, rect, 20, false);
        ui.add_space(24.0);
        ui.vertical(|ui| {
            ui.set_max_width(ui.available_width().min(760.0));
            ui.add_space((width * 1.5 - 210.0).max(0.0));
            ui.add(egui::Label::new(RichText::new(state.title()).font(theme::heavy_font(34.0))).wrap());
            let authors = state.authors();
            if !authors.is_empty() {
                ui.label(RichText::new(authors).size(15.0).color(palette.secondary));
            }
            if state.detail.as_ref().map(|d| d.official).unwrap_or(false) {
                ui.label(RichText::new(format!("{}  {}", icon::SEAL_CHECK, t("Official translation"))).size(13.0).color(palette.accent));
            }
            ui.add_space(12.0);
            ui.allocate_ui(vec2(ui.available_width().min(560.0), 68.0), |ui| fact_strip(ui, state, palette));
        });
    });
}

fn fact_strip(ui: &mut Ui, state: &DetailState, palette: &Palette) {
    let facts: Vec<(String, &str)> = [
        (if state.chapters.is_empty() { "-".to_string() } else { state.chapters.len().to_string() }, t("CHAPTERS")),
        (state.status(), t("STATUS")),
        (state.kind(), t("TYPE")),
        (state.year(), t("YEAR")),
    ]
    .into_iter()
    .filter(|(v, _)| !v.is_empty())
    .collect();

    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(width, 68.0), Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::same(22), palette.glass_strong.gamma_multiply(0.75));
    ui.painter().rect_stroke(rect, CornerRadius::same(22), Stroke::new(1.0, palette.glass_stroke), StrokeKind::Inside);
    let cell = rect.width() / facts.len().max(1) as f32;
    for (index, (value, label)) in facts.iter().enumerate() {
        let center = pos2(rect.left() + cell * (index as f32 + 0.5), rect.center().y);
        if index > 0 {
            let x = rect.left() + cell * index as f32;
            ui.painter().vline(x, (rect.center().y - 14.0)..=(rect.center().y + 14.0), Stroke::new(1.0, palette.glass_stroke));
        }
        ui.painter().text(center - vec2(0.0, 8.0), Align2::CENTER_CENTER, value, theme::bold_font(16.5), palette.text);
        ui.painter().text(center + vec2(0.0, 13.0), Align2::CENTER_CENTER, *label, theme::bold_font(10.0), palette.secondary);
    }
}

fn progress_card(ui: &mut Ui, state: &mut DetailState, shared: &mut Shared, progress: &HashMap<String, ChapterProgress>, palette: &Palette) {
    let read = progress.values().filter(|p| p.completed).count();
    let total = state.chapters.len().max(read);
    let fraction = if total > 0 { read as f32 / total as f32 } else { 0.0 };
    let saved = shared.store.saved(&state.manga.id).cloned();
    let target = state.resume_target(shared);

    theme::card_frame(palette).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            widgets::progress_ring(ui, fraction, 66.0, 7.0);
            ui.add_space(8.0);
            ui.vertical(|ui| {
                ui.add_space(8.0);
                ui.label(RichText::new(if total > 0 { tf("{read} of {total} chapters read", &[("read", &read), ("total", &total)]) } else { t("Not started").to_string() }).font(theme::bold_font(17.0)));
                let subtitle = match &saved {
                    Some(s) if s.has_history && s.last_page_count > 0 => {
                        tf("Last read: {chapter}, page {page} of {count}", &[("chapter", &s.last_chapter_name), ("page", &(s.last_page + 1)), ("count", &s.last_page_count)])
                    }
                    Some(s) if s.has_history => tf("Last read: {chapter}", &[("chapter", &s.last_chapter_name)]),
                    _ => t("Click Start to begin with the first chapter.").to_string(),
                };
                ui.add(egui::Label::new(RichText::new(subtitle).size(13.0).color(palette.secondary)).wrap());
            });
        });
        ui.add_space(14.0);
        ui.horizontal(|ui| {
            let label = match (&target, saved.as_ref().map(|s| s.has_history).unwrap_or(false)) {
                (Some(target), true) => tf("Continue · {chapter}", &[("chapter", &target.name)]),
                (Some(target), false) => tf("Start · {chapter}", &[("chapter", &target.name)]),
                (None, _) => t("Read").to_string(),
            };
            let width = ui.available_width() - 2.0 * 44.0 - 2.0 * ui.spacing().item_spacing.x;
            if widgets::primary_button(ui, &label, Some(icon::PLAY), Some(width), target.is_some()).clicked() {
                if let Some(target) = &target {
                    shared.router.open_reader(target, &state.chapters, &state.manga.id, &state.title());
                }
            }
            let in_library = shared.store.in_library(&state.manga.id);
            let bookmark = if in_library {
                widgets::icon_button(ui, icon::BOOKMARK_FILL, 44.0, Some(palette.accent))
            } else {
                widgets::icon_button(ui, icon::BOOKMARK_SIMPLE, 44.0, None)
            };
            if bookmark
                .on_hover_text(if in_library { t("Remove from library") } else { t("Add to library") })
                .clicked()
            {
                let (title, authors, status, kind) = (state.title(), state.authors(), state.status(), state.kind());
                shared.store.toggle_library(&state.manga.id, &title, &authors, &status, &kind);
            }
            if widgets::icon_button(ui, icon::DOWNLOAD_SIMPLE, 44.0, None).on_hover_text(t("Download unread chapters")).clicked() {
                request_download(state, shared, true);
            }
            let anilist_id = shared.store.saved(&state.manga.id).and_then(|m| m.anilist_id).or(state.detail.as_ref().and_then(|d| d.anilist_id));
            if let (Some(token), Some(id)) = (shared.anilist_token.clone(), anilist_id) {
                let favourite = shared.anilist.status().favourites.contains(&id);
                let hint = if favourite { t("Remove from AniList favourites") } else { t("Add to AniList favourites") };
                let heart = if favourite {
                    widgets::icon_button(ui, icon::HEART_FILL, 44.0, Some(palette.accent))
                } else {
                    widgets::icon_button(ui, icon::HEART, 44.0, None)
                };
                if heart.on_hover_text(hint).clicked() {
                    shared.anilist.toggle_favourite(&token, id);
                }
            }
        });
    });
}

// marks chapters up to the anilist progress as read, once
fn apply_anilist_progress(manga_id: &str, chapters: &[Chapter], shared: &mut Shared) {
    let Some(progress) = shared.store.saved(manga_id).and_then(|m| m.anilist_progress) else { return };
    for chapter in chapters {
        let read = shared.store.progress.get(&chapter.id).is_some_and(|p| p.completed);
        if !read && crate::anilist::chapter_number(&chapter.name).is_some_and(|n| n <= progress) {
            shared.store.set_read(manga_id, chapter, true);
        }
    }
    if let Some(m) = shared.store.manga.get_mut(manga_id) {
        m.anilist_progress = None;
    }
}

fn about_card(ui: &mut Ui, state: &mut DetailState, palette: &Palette) {
    let Some(detail) = state.detail.clone() else { return };
    if detail.summary.is_empty() && detail.tags.is_empty() {
        return;
    }
    theme::card_frame(palette).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(RichText::new(t("SYNOPSIS")).font(theme::bold_font(11.0)).color(palette.secondary));
        ui.add_space(4.0);
        if !detail.summary.is_empty() {
            let text = if state.summary_expanded || detail.summary.chars().count() <= 320 {
                detail.summary.clone()
            } else {
                format!("{}...", detail.summary.chars().take(320).collect::<String>().trim_end())
            };
            ui.add(egui::Label::new(RichText::new(text).size(14.5)).wrap());
            if detail.summary.chars().count() > 320
                && widgets::link_button(ui, if state.summary_expanded { t("Show less") } else { t("Show more") }).clicked()
            {
                state.summary_expanded = !state.summary_expanded;
            }
        }
        if !detail.tags.is_empty() {
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                for tag in &detail.tags {
                    widgets::chip(ui, tag, None);
                }
            });
        }
    });
}

fn chapters(
    ui: &mut Ui,
    state: &mut DetailState,
    shared: &mut Shared,
    progress: &HashMap<String, ChapterProgress>,
    downloaded: &std::collections::HashSet<String>,
    palette: &Palette,
) {
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            let title = if state.chapters.is_empty() { t("Chapters").to_string() } else { tf("{n} chapters", &[("n", &state.chapters.len())]) };
            ui.label(RichText::new(title).font(theme::bold_font(22.0)));
            if !downloaded.is_empty() {
                ui.label(RichText::new(tf("{n} downloaded", &[("n", &downloaded.len())])).size(12.5).color(palette.secondary));
            }
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let label = if state.downloaded_only { t("Downloaded") } else if state.oldest_first { t("Oldest first") } else { t("Newest first") };
            ui.menu_button(RichText::new(format!("{}  {label}", icon::FUNNEL_SIMPLE)).size(14.0), |ui| {
                if ui.selectable_label(!state.oldest_first, t("Newest first")).clicked() {
                    state.oldest_first = false;
                    ui.close();
                }
                if ui.selectable_label(state.oldest_first, t("Oldest first")).clicked() {
                    state.oldest_first = true;
                    ui.close();
                }
                ui.separator();
                ui.checkbox(&mut state.downloaded_only, t("Downloaded only"));
            });
        });
    });
    ui.add_space(8.0);

    let mut visible: Vec<Chapter> = state.chapters.clone();
    if state.downloaded_only {
        visible.retain(|c| downloaded.contains(&c.id));
    }
    if state.oldest_first {
        visible.reverse();
    }

    if visible.is_empty() {
        if !state.chapters.is_empty() && state.downloaded_only {
            widgets::empty_state(ui, icon::DOWNLOAD_SIMPLE, t("No downloaded chapters"), "");
            ui.vertical_centered(|ui| {
                if widgets::link_button(ui, t("Show all chapters")).clicked() {
                    state.downloaded_only = false;
                }
            });
        } else if state.loading.is_some() {
            widgets::spinner(ui);
        } else if !net::is_online() {
            widgets::empty_state(ui, icon::WIFI_SLASH, t("Not available offline"), t("This series hasn't been opened before, so its chapters aren't saved."));
        } else if let Some(error) = state.error.clone() {
            if widgets::error_state(ui, &error) {
                state.loaded_once = false;
            }
        } else {
            widgets::empty_state(ui, icon::BOOK_OPEN, t("No chapters yet"), "");
        }
        return;
    }

    let online = net::is_online();
    let row_height = 68.0;
    let spacing = 8.0;
    // only lay out visible rows, long series have over a thousand chapters
    let total_height = visible.len() as f32 * (row_height + spacing);
    let width = ui.available_width();
    let (area, _) = ui.allocate_exact_size(vec2(width, total_height), Sense::hover());
    let clip = ui.clip_rect();
    let first = (((clip.top() - area.top()) / (row_height + spacing)).floor().max(0.0)) as usize;
    let last = ((((clip.bottom() - area.top()) / (row_height + spacing)).ceil()) as usize + 1).min(visible.len());

    for index in first..last {
        let chapter = &visible[index];
        let rect = Rect::from_min_size(pos2(area.left(), area.top() + index as f32 * (row_height + spacing)), vec2(width, row_height));
        let item = progress.get(&chapter.id);
        let is_downloaded = downloaded.contains(&chapter.id);
        chapter_row(ui, rect, chapter, item, is_downloaded, !online && !is_downloaded, state, shared, palette);
    }
}

#[allow(clippy::too_many_arguments)]
fn chapter_row(
    ui: &mut Ui,
    rect: Rect,
    chapter: &Chapter,
    progress: Option<&ChapterProgress>,
    is_downloaded: bool,
    unavailable: bool,
    state: &mut DetailState,
    shared: &mut Shared,
    palette: &Palette,
) {
    let is_read = progress.map(|p| p.completed).unwrap_or(false);
    let partial = progress.filter(|p| !p.completed && p.page_count > 0);
    let control = Rect::from_center_size(rect.right_center() - vec2(30.0, 0.0), vec2(44.0, 44.0));
    let body = Rect::from_min_max(rect.min, pos2(control.left() - 4.0, rect.bottom()));

    let response = ui.interact(body, ui.id().with(("chapter", &chapter.id)), if unavailable { Sense::hover() } else { Sense::click() });
    let hover = ui.ctx().animate_bool_with_time(response.id, response.hovered() && !unavailable, 0.12);
    let fill = widgets::lerp_color(palette.glass_strong.gamma_multiply(0.55), palette.glass_strong.gamma_multiply(0.95), hover);
    ui.painter().rect_filled(rect, CornerRadius::same(18), fill);

    let dim = if is_read || unavailable { 0.5 } else { 1.0 };
    let text = palette.text.gamma_multiply(dim);
    let secondary = palette.secondary.gamma_multiply(dim);

    // "Chapter 1198" becomes a big number badge
    let words: Vec<&str> = chapter.name.split_whitespace().collect();
    let number_split = words.last().filter(|w| words.len() >= 2 && w.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false));
    let badge_center = pos2(rect.left() + 46.0, rect.center().y);
    let text_left = rect.left() + 92.0;
    if let Some(number) = number_split {
        let kind = words[..words.len() - 1].join(" ").to_uppercase();
        ui.painter().text(badge_center - vec2(0.0, 11.0), Align2::CENTER_CENTER, kind, theme::bold_font(9.5), secondary);
        ui.painter().text(badge_center + vec2(0.0, 7.0), Align2::CENTER_CENTER, *number, theme::heavy_font(22.0), text);
        if let Some(date) = chapter.date {
            ui.painter().text(pos2(text_left, rect.center().y - 10.0), Align2::LEFT_CENTER, short_date(date), theme::bold_font(14.5), text);
        }
    } else {
        ui.painter().text(badge_center, Align2::CENTER_CENTER, icon::BOOK_OPEN_TEXT, theme::icon_font(22.0), secondary);
        ui.painter().text(pos2(text_left, rect.center().y - 10.0), Align2::LEFT_CENTER, &chapter.name, theme::bold_font(14.5), text);
    }
    let status_line = match partial {
        Some(p) => tf("Page {page} of {count}", &[("page", &(p.page + 1)), ("count", &p.page_count)]),
        None if is_read => t("Read").to_string(),
        None => t("Not read yet").to_string(),
    };
    let status_color = if partial.is_some() { palette.accent } else { secondary };
    ui.painter().text(pos2(text_left, rect.center().y + 11.0), Align2::LEFT_CENTER, status_line, theme::font(12.5), status_color);
    if is_read {
        ui.painter().text(pos2(body.right() - 18.0, rect.center().y), Align2::CENTER_CENTER, icon::CHECK_CIRCLE, theme::icon_font(20.0), palette.accent);
    }
    if let Some(p) = partial {
        let bar = Rect::from_min_size(pos2(rect.left() + 16.0, rect.bottom() - 7.0), vec2(rect.width() - 32.0, 3.0));
        widgets::paint_progress_bar(ui, bar, (p.page + 1) as f32 / p.page_count.max(1) as f32, *palette);
    }

    if response.clicked() {
        shared.router.open_reader(chapter, &state.chapters, &state.manga.id, &state.title());
    }
    let response = response.on_hover_cursor(if unavailable { egui::CursorIcon::NotAllowed } else { egui::CursorIcon::PointingHand });
    response.context_menu(|ui| {
        if ui.button(if is_read { format!("{}  {}", icon::CIRCLE, t("Mark as unread")) } else { format!("{}  {}", icon::CHECK_CIRCLE, t("Mark as read")) }).clicked() {
            shared.store.set_read(&state.manga.id, chapter, !is_read);
            ui.close();
        }
        if is_downloaded {
            if ui.button(format!("{}  {}", icon::TRASH, t("Delete download"))).clicked() {
                if let Some(item) = shared.store.downloads.remove(&chapter.id) {
                    shared.store.mark_dirty();
                    std::thread::spawn(move || crate::images::disk::remove_downloads(&item.page_urls));
                }
                ui.close();
            }
        } else if !shared.downloads.is_busy(&chapter.id) && ui.button(format!("{}  {}", icon::DOWNLOAD_SIMPLE, t("Download"))).clicked() {
            shared.downloads.download(std::slice::from_ref(chapter), &state.manga.id, &state.title());
            ui.close();
        }
    });

    download_control(ui, control, chapter, is_downloaded, state, shared, palette);
}

fn download_control(ui: &mut Ui, rect: Rect, chapter: &Chapter, is_downloaded: bool, state: &DetailState, shared: &mut Shared, palette: &Palette) {
    let response = ui.interact(rect, ui.id().with(("dl", &chapter.id)), Sense::click());
    let center = rect.center();
    if is_downloaded {
        ui.painter().text(center, Align2::CENTER_CENTER, icon::CHECK_FAT, theme::icon_font(18.0), palette.accent);
        response.on_hover_text(t("Downloaded"));
        return;
    }
    if let Some(fraction) = shared.downloads.progress(&chapter.id) {
        ui.painter().circle_stroke(center, 11.0, Stroke::new(3.0, palette.quaternary.gamma_multiply(2.0)));
        let steps = 40;
        let points: Vec<egui::Pos2> = (0..=steps)
            .map(|i| {
                let angle = -std::f32::consts::FRAC_PI_2 + std::f32::consts::TAU * fraction.max(0.03) * i as f32 / steps as f32;
                center + egui::Vec2::angled(angle) * 11.0
            })
            .collect();
        ui.painter().add(egui::Shape::line(points, Stroke::new(3.0, palette.accent)));
        ui.painter().rect_filled(Rect::from_center_size(center, vec2(6.0, 6.0)), CornerRadius::same(1), palette.accent);
        if response.on_hover_text(t("Cancel download")).clicked() {
            shared.downloads.cancel(&chapter.id);
        }
        return;
    }
    if shared.downloads.is_queued(&chapter.id) {
        ui.painter().text(center, Align2::CENTER_CENTER, icon::CLOCK, theme::icon_font(20.0), palette.secondary);
        if response.on_hover_text(t("Waiting, click to cancel")).clicked() {
            shared.downloads.cancel(&chapter.id);
        }
        return;
    }
    let failed = shared.downloads.has_failed(&chapter.id);
    let hover = response.hovered();
    ui.painter().text(
        center,
        Align2::CENTER_CENTER,
        if failed { icon::WARNING_CIRCLE } else { icon::DOWNLOAD_SIMPLE },
        theme::font(20.0),
        if hover { palette.accent } else { palette.secondary },
    );
    if response.on_hover_text(if failed { t("Download failed, click to retry") } else { t("Download") }).clicked() {
        shared.downloads.download(std::slice::from_ref(chapter), &state.manga.id, &state.title());
    }
}

fn request_download(state: &mut DetailState, shared: &Shared, unread_only: bool) {
    // oldest first so chapters arrive in reading order
    let candidates: Vec<Chapter> = state
        .chapters
        .iter()
        .rev()
        .filter(|c| !shared.store.downloads.contains_key(&c.id) && !shared.downloads.is_busy(&c.id))
        .filter(|c| !unread_only || !shared.store.progress.get(&c.id).map(|p| p.completed).unwrap_or(false))
        .cloned()
        .collect();
    if !candidates.is_empty() {
        state.confirm_download = Some(candidates);
    }
}

fn dialogs(ui: &mut Ui, state: &mut DetailState, shared: &mut Shared, palette: &Palette) {
    if let Some(list) = state.confirm_download.clone() {
        let mut close = false;
        let modal = egui::Modal::new(egui::Id::new(("confirm-download", &state.manga.id)))
            .frame(theme::modal_frame(palette))
            .show(ui.ctx(), |ui| {
                ui.set_width(380.0);
                ui.label(RichText::new(tf("Download {n} chapters?", &[("n", &list.len())])).font(theme::bold_font(20.0)));
                ui.label(RichText::new(t("They are saved on this PC so you can read them without a connection.")).color(palette.secondary));
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    if widgets::glass_button(ui, t("Cancel"), None, Some(130.0)).clicked() {
                        close = true;
                    }
                    if widgets::primary_button(ui, t("Download"), Some(icon::DOWNLOAD_SIMPLE), Some(170.0), true).clicked() {
                        shared.downloads.download(&list, &state.manga.id, &state.title());
                        close = true;
                    }
                });
            });
        if close || modal.should_close() {
            state.confirm_download = None;
        }
    }

    if state.confirm_delete {
        let mut close = false;
        let modal = egui::Modal::new(egui::Id::new(("confirm-delete", &state.manga.id)))
            .frame(theme::modal_frame(palette))
            .show(ui.ctx(), |ui| {
                ui.set_width(380.0);
                ui.label(RichText::new(t("Delete all downloaded chapters of this series?")).font(theme::bold_font(18.0)));
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    if widgets::glass_button(ui, t("Cancel"), None, Some(130.0)).clicked() {
                        close = true;
                    }
                    if widgets::primary_button(ui, t("Delete downloads"), Some(icon::TRASH), Some(200.0), true).clicked() {
                        super::library::delete_series_downloads(shared, &state.manga.id);
                        close = true;
                    }
                });
            });
        if close || modal.should_close() {
            state.confirm_delete = false;
        }
    }
}
