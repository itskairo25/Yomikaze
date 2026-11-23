use std::collections::HashMap;

use egui::{Align2, Color32, CornerRadius, Rect, RichText, Sense, Ui, pos2, vec2};
use crate::icons as icon;
use poll_promise::Promise;

use super::{grid, page, widgets};
use crate::app::{Shared, Tab, spawn};
use crate::images::Kind;
use crate::model::{MangaSummary, ResultPage};
use crate::net;
use crate::source::{self, SearchSort};
use crate::theme::{self, Palette};
use crate::i18n::t;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Feed {
    #[default]
    Hot,
    Latest,
    Popular,
    Newest,
}

impl Feed {
    const ALL: [Feed; 4] = [Feed::Hot, Feed::Latest, Feed::Popular, Feed::Newest];

    fn title(self) -> &'static str {
        match self {
            Feed::Hot => t("Hot"),
            Feed::Latest => t("Latest"),
            Feed::Popular => t("Popular"),
            Feed::Newest => t("New"),
        }
    }

    fn glyph(self) -> &'static str {
        match self {
            Feed::Hot => icon::FIRE,
            Feed::Latest => icon::CLOCK,
            Feed::Popular => icon::STAR,
            Feed::Newest => icon::SPARKLE,
        }
    }

    fn ranked_title(self) -> &'static str {
        match self {
            Feed::Hot => t("Trending now"),
            Feed::Latest => t("Fresh chapters"),
            Feed::Popular => t("All-time top"),
            Feed::Newest => t("Just added"),
        }
    }

    fn hero_caption(self) -> &'static str {
        match self {
            Feed::Hot => t("HOT RIGHT NOW"),
            Feed::Latest => t("JUST UPDATED"),
            Feed::Popular => t("MOST POPULAR"),
            Feed::Newest => t("NEW ARRIVAL"),
        }
    }
}

#[derive(Default)]
struct FeedState {
    items: Vec<MangaSummary>,
    has_more: bool,
    next_page: usize,
    loaded: bool,
    error: Option<String>,
    request: Option<(bool, Promise<source::Result<ResultPage>>)>,
}

#[derive(Default)]
pub struct BrowseState {
    feed: Feed,
    feeds: HashMap<Feed, FeedState>,
    adult: bool,
}

fn load(ui: &Ui, feed: Feed, state: &mut FeedState, reset: bool, adult: bool) {
    if state.request.is_some() || !net::is_online() {
        return;
    }
    if !reset && !state.has_more {
        return;
    }
    let page = if reset { 1 } else { state.next_page.max(1) };
    let offset = (page - 1) * source::PAGE_SIZE;
    let promise = spawn(ui.ctx(), move || match feed {
        Feed::Hot => source::hot_updates().map(|items| ResultPage { items, has_more: false }),
        Feed::Latest => source::latest_updates(page),
        Feed::Popular => source::search("", SearchSort::Popularity, None, offset, adult),
        Feed::Newest => source::search("", SearchSort::RecentlyAdded, None, offset, adult),
    });
    state.request = Some((reset, promise));
}

fn poll(state: &mut FeedState) {
    let Some((reset, promise)) = &state.request else { return };
    let Some(result) = promise.ready() else { return };
    match result {
        Ok(page) => {
            if *reset {
                state.items = page.items.clone();
                state.next_page = 2;
            } else {
                for item in &page.items {
                    if !state.items.iter().any(|m| m.id == item.id) {
                        state.items.push(item.clone());
                    }
                }
                state.next_page += 1;
            }
            state.has_more = page.has_more && !page.items.is_empty();
            state.error = None;
        }
        Err(error) => state.error = Some(error.clone()),
    }
    state.loaded = true;
    state.request = None;
}

pub fn show(ui: &mut Ui, state: &mut BrowseState, shared: &mut Shared) {
    let palette = Palette::of(ui.ctx());
    let adult = shared.store.settings.show_adult;
    if adult != state.adult {
        state.feeds.clear();
        state.adult = adult;
    }
    let feed = state.feed;
    let current = state.feeds.entry(feed).or_default();
    poll(current);
    if !current.loaded && current.request.is_none() {
        load(ui, feed, current, true, adult);
    }
    if current.items.is_empty() && current.error.is_some() && net::is_online() && current.request.is_none() && !current.loaded {
        load(ui, feed, current, true, adult);
    }

    page(ui, "browse", |ui| {
        ui.horizontal(|ui| {
            widgets::page_title(ui, t("Browse"));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if widgets::icon_button(ui, icon::ARROWS_CLOCKWISE, 38.0, None).on_hover_text(t("Refresh")).clicked() {
                    let current = state.feeds.entry(feed).or_default();
                    load(ui, feed, current, true, adult);
                }
            });
        });

        ui.horizontal(|ui| {
            let width = ((ui.available_width() - 30.0) / 4.0).min(190.0);
            for option in Feed::ALL {
                if widgets::pill(ui, option.title(), Some(option.glyph()), option == state.feed, width).clicked() {
                    state.feed = option;
                }
            }
        });
        ui.add_space(20.0);

        let feed = state.feed;
        let current = state.feeds.entry(feed).or_default();
        content(ui, feed, current, shared, &palette, adult);
    });
}

fn content(ui: &mut Ui, feed: Feed, state: &mut FeedState, shared: &mut Shared, palette: &Palette, adult: bool) {
    if state.items.is_empty() {
        if !net::is_online() {
            widgets::empty_state(ui, icon::WIFI_SLASH, t("You're offline"), t("Browsing needs a connection. Your downloaded chapters are in the Library."));
            ui.vertical_centered(|ui| {
                if widgets::primary_button(ui, t("Open Library"), None, Some(170.0), true).clicked() {
                    shared.router.select(Tab::Library);
                }
            });
        } else if let Some(error) = state.error.clone() {
            if widgets::error_state(ui, &error) {
                load(ui, feed, state, true, adult);
            }
        } else if state.loaded {
            widgets::empty_state(ui, icon::TRAY, t("Nothing here yet"), "");
        } else {
            widgets::spinner(ui);
        }
        return;
    }

    let items = state.items.clone();
    let ranked: Vec<_> = items.iter().skip(1).take(6).cloned().collect();
    let rest: Vec<_> = items.iter().skip(1 + ranked.len()).cloned().collect();

    if let Some(featured) = items.first() {
        if hero(ui, shared, featured, feed.hero_caption(), palette).clicked() {
            shared.router.open(featured);
        }
        ui.add_space(26.0);
    }

    if !ranked.is_empty() {
        ui.label(RichText::new(feed.ranked_title()).font(theme::bold_font(22.0)));
        ui.add_space(8.0);
        // as many as fit in one row
        let gap = 18.0;
        let fits = ((ui.available_width() + gap) / (150.0 + gap)).floor().clamp(1.0, ranked.len() as f32) as usize;
        let slot = (ui.available_width() - gap * (fits as f32 - 1.0)) / fits as f32;
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for (index, manga) in ranked.iter().take(fits).enumerate() {
                if ranked_card(ui, shared, manga, index + 2, slot, palette).clicked() {
                    shared.router.open(manga);
                }
            }
        });
        ui.add_space(24.0);
    }

    if !rest.is_empty() {
        ui.label(RichText::new(t("More to read")).font(theme::bold_font(22.0)));
        ui.add_space(10.0);
        let last = rest.len() - 1;
        let mut reached_end = false;
        grid(ui, rest.len(), 150.0, 18.0, |ui, index, width| {
            let manga = &rest[index];
            let response = browse_card(ui, shared, manga, width, palette);
            if response.clicked() {
                shared.router.open(manga);
            }
            if index == last && ui.is_rect_visible(response.rect) {
                reached_end = true;
            }
        });
        if reached_end {
            load(ui, feed, state, false, adult);
        }
    }

    if state.request.is_some() {
        widgets::spinner(ui);
    }
}

fn hero(ui: &mut Ui, shared: &mut Shared, manga: &MangaSummary, caption: &str, palette: &Palette) -> egui::Response {
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(vec2(width, 260.0), Sense::click());
    let radius = 30;
    ui.painter().rect_filled(rect, CornerRadius::same(radius), Color32::from_rgb(20, 18, 32));
    widgets::paint_blurred_cover(ui, &mut shared.images, &manga.id, rect, radius, 1.0);
    ui.painter().rect_filled(rect, CornerRadius::same(radius), Color32::from_black_alpha(100));
    theme::radial_glow(ui.painter(), rect.left_center() + vec2(160.0, 0.0), 300.0, palette.accent.gamma_multiply(0.35));

    let cover = Rect::from_min_size(rect.min + vec2(28.0, 24.0), vec2(141.0, 212.0));
    widgets::paint_cover(ui, &mut shared.images, &manga.id, cover, 14, false);

    let left = cover.right() + 28.0;
    let text_width = rect.right() - left - 32.0;
    let painter = ui.painter();
    painter.text(pos2(left, rect.top() + 46.0), Align2::LEFT_TOP, caption, theme::bold_font(12.0), Color32::from_white_alpha(190));
    let title = painter.layout(manga.title.clone(), theme::heavy_font(30.0), Color32::WHITE, text_width);
    let title_height = title.size().y.min(112.0);
    painter.with_clip_rect(Rect::from_min_size(pos2(left, rect.top() + 68.0), vec2(text_width, 112.0))).galley(pos2(left, rect.top() + 68.0), title, Color32::WHITE);
    let line = manga.latest_chapter.clone().unwrap_or_else(|| manga.meta_line());
    painter.text(pos2(left, rect.top() + 76.0 + title_height), Align2::LEFT_TOP, line, theme::font(14.5), Color32::from_white_alpha(210));

    let button = Rect::from_min_size(pos2(left, (rect.top() + 112.0 + title_height).min(rect.bottom() - 62.0)), vec2(118.0, 40.0));
    let hover = ui.ctx().animate_bool_with_time(response.id.with("h"), response.hovered(), 0.15);
    painter.rect_filled(button, CornerRadius::same(255), Color32::from_white_alpha((40.0 + 30.0 * hover) as u8));
    painter.rect_stroke(button, CornerRadius::same(255), egui::Stroke::new(1.0, Color32::from_white_alpha(60)), egui::StrokeKind::Inside);
    painter.text(button.center(), Align2::CENTER_CENTER, format!("{}  {}", icon::BOOK_OPEN, t("Open")), theme::bold_font(14.5), Color32::WHITE);
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn ranked_card(ui: &mut Ui, shared: &mut Shared, manga: &MangaSummary, rank: usize, slot: f32, palette: &Palette) -> egui::Response {
    let number_width = (slot * 0.3).clamp(40.0, 58.0);
    let cover_width = (slot - number_width).min(170.0);
    let (rect, response) = ui.allocate_exact_size(vec2(number_width + cover_width, cover_width * 1.5 + 46.0), Sense::click());
    let cover = Rect::from_min_size(pos2(rect.left() + number_width, rect.top()), vec2(cover_width, cover_width * 1.5));
    ui.painter().text(
        pos2(rect.left() + number_width + 10.0, cover.bottom() + 6.0),
        Align2::RIGHT_BOTTOM,
        rank.to_string(),
        theme::heavy_font(82.0),
        palette.accent.gamma_multiply(0.9),
    );
    widgets::paint_cover(ui, &mut shared.images, &manga.id, cover, 14, false);
    let hover = ui.ctx().animate_bool_with_time(response.id.with("h"), response.hovered(), 0.15);
    if hover > 0.0 {
        ui.painter().rect_filled(cover, CornerRadius::same(14), Color32::from_white_alpha((18.0 * hover) as u8));
    }
    let painter = ui.painter();
    widgets::paint_line(painter, pos2(cover.left(), cover.bottom() + 8.0), &manga.title, theme::bold_font(13.5), palette.text, cover_width);
    let line = manga.latest_chapter.clone().unwrap_or_else(|| manga.meta_line());
    widgets::paint_line(painter, pos2(cover.left(), cover.bottom() + 27.0), &line, theme::font(12.0), palette.secondary, cover_width);
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn browse_card(ui: &mut Ui, shared: &mut Shared, manga: &MangaSummary, width: f32, palette: &Palette) -> egui::Response {
    let response = widgets::manga_card(
        ui,
        &mut shared.images,
        manga,
        manga
            .status
            .clone()
            .or_else(|| manga.updated_at.map(crate::model::relative_time))
            .as_deref(),
        width,
    );
    let badge = manga
        .latest_chapter
        .as_ref()
        .map(|c| c.replace("Chapter", "Ch.").replace("Episode", "Ep."))
        .or_else(|| manga.kind.clone());
    if let Some(badge) = badge {
        let cover = Rect::from_min_size(response.rect.min, vec2(width, width * 1.5));
        let galley = ui.painter().layout_no_wrap(badge, theme::bold_font(11.5), Color32::WHITE);
        let pill = Rect::from_min_size(
            pos2(cover.left() + 8.0, cover.bottom() - galley.size().y - 18.0),
            galley.size() + vec2(16.0, 8.0),
        );
        ui.painter().rect_filled(pill, CornerRadius::same(255), Color32::from_black_alpha(165));
        ui.painter().galley(pill.min + vec2(8.0, 4.0), galley, Color32::WHITE);
    }
    let _ = (palette, Kind::Cover);
    response
}
