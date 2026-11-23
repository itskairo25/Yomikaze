use std::time::{Duration, Instant};

use egui::{Align2, Color32, CornerRadius, Rect, RichText, Sense, Ui, vec2};
use crate::icons as icon;
use poll_promise::Promise;

use super::{grid, page, widgets};
use crate::app::{Shared, spawn};
use crate::model::{MangaSummary, ResultPage};
use crate::net;
use crate::source::{self, SearchSort};
use crate::theme::{self, Palette};
use crate::i18n::{t, tf};

const GENRES: [(&str, &str); 22] = [
    ("Action", icon::FIRE),
    ("Adventure", icon::MAP_TRIFOLD),
    ("Comedy", icon::SMILEY),
    ("Drama", icon::MASK_SAD),
    ("Fantasy", icon::MAGIC_WAND),
    ("Historical", icon::BANK),
    ("Horror", icon::GHOST),
    ("Isekai", icon::GLOBE_HEMISPHERE_EAST),
    ("Martial Arts", icon::HAND_FIST),
    ("Mecha", icon::ROBOT),
    ("Mystery", icon::MAGNIFYING_GLASS),
    ("Psychological", icon::BRAIN),
    ("Romance", icon::HEART),
    ("School Life", icon::GRADUATION_CAP),
    ("Sci-fi", icon::ATOM),
    ("Seinen", icon::USER),
    ("Shoujo", icon::SPARKLE),
    ("Shounen", icon::LIGHTNING),
    ("Slice of Life", icon::COFFEE),
    ("Sports", icon::PERSON_SIMPLE_RUN),
    ("Supernatural", icon::EYE),
    ("Tragedy", icon::CLOUD_RAIN),
];

pub struct SearchState {
    query: String,
    tag: Option<String>,
    sort: SearchSort,
    results: Vec<MangaSummary>,
    has_more: bool,
    error: Option<String>,
    key: String,
    // for the typing pause before searching
    changed_at: Option<Instant>,
    request: Option<(String, bool, Promise<source::Result<ResultPage>>)>,
}

impl Default for SearchState {
    fn default() -> Self {
        Self {
            query: String::new(),
            tag: None,
            sort: SearchSort::BestMatch,
            results: Vec::new(),
            has_more: false,
            error: None,
            key: String::new(),
            changed_at: None,
            request: None,
        }
    }
}

impl SearchState {
    fn trimmed(&self) -> String {
        self.query.trim().to_string()
    }

    fn active(&self) -> bool {
        !self.trimmed().is_empty() || self.tag.is_some()
    }

    fn effective_sort(&self) -> SearchSort {
        if self.trimmed().is_empty() && self.sort == SearchSort::BestMatch { SearchSort::Popularity } else { self.sort }
    }

    fn wanted_key(&self, adult: bool) -> String {
        format!("{}|{}|{}|{adult}", self.trimmed(), self.tag.clone().unwrap_or_default(), self.sort.label())
    }

    fn start(&mut self, ui: &Ui, adult: bool, more: bool) {
        let key = self.wanted_key(adult);
        let text = self.trimmed();
        let tag = self.tag.clone();
        let sort = self.effective_sort();
        let offset = if more { self.results.len() } else { 0 };
        let promise = spawn(ui.ctx(), move || source::search(&text, sort, tag.as_deref(), offset, adult));
        self.request = Some((key, more, promise));
    }

    fn poll(&mut self) {
        let Some((key, more, promise)) = &self.request else { return };
        let Some(result) = promise.ready() else { return };
        match result {
            Ok(page) => {
                if *more {
                    for item in &page.items {
                        if !self.results.iter().any(|m| m.id == item.id) {
                            self.results.push(item.clone());
                        }
                    }
                } else {
                    self.results = page.items.clone();
                }
                self.has_more = page.has_more && !page.items.is_empty();
                self.error = None;
            }
            Err(error) => {
                if !*more {
                    self.results.clear();
                }
                self.has_more = false;
                self.error = Some(error.clone());
            }
        }
        self.key = key.clone();
        self.request = None;
    }
}

pub fn show(ui: &mut Ui, state: &mut SearchState, shared: &mut Shared) {
    let palette = Palette::of(ui.ctx());
    let adult = shared.store.settings.show_adult;
    if let Some(text) = shared.router.search_text.take() {
        state.query = text;
    }
    state.poll();

    let wanted = state.wanted_key(adult);
    if state.active() && net::is_online() && wanted != state.key && state.request.as_ref().map(|r| r.0 != wanted).unwrap_or(true) {
        let waited = state.changed_at.map(|t| t.elapsed() >= Duration::from_millis(350)).unwrap_or(true);
        if waited {
            state.start(ui, adult, false);
        } else {
            ui.ctx().request_repaint_after(Duration::from_millis(120));
        }
    }

    page(ui, "search", |ui| {
        ui.horizontal(|ui| {
            widgets::page_title(ui, t(state.tag.as_deref().unwrap_or("Search")));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if state.active() {
                    ui.menu_button(RichText::new(format!("{}  {}", icon::SORT_ASCENDING, t(state.sort.label()))).size(14.5), |ui| {
                        for option in SearchSort::ALL {
                            if ui.selectable_label(state.sort == option, t(option.label())).clicked() {
                                state.sort = option;
                                ui.close();
                            }
                        }
                    });
                }
                if state.tag.is_some() && widgets::glass_button(ui, t("Clear genre"), Some(icon::X), None).clicked() {
                    state.tag = None;
                }
            });
        });

        search_field(ui, state, shared, &palette);
        ui.add_space(18.0);

        if state.active() {
            results(ui, state, shared, adult);
        } else {
            discover(ui, state, shared, &palette);
        }
    });
}

fn search_field(ui: &mut Ui, state: &mut SearchState, shared: &mut Shared, palette: &Palette) {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(width, 52.0), Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::same(255), palette.glass_strong.gamma_multiply(0.85));
    ui.painter().rect_stroke(rect, CornerRadius::same(255), egui::Stroke::new(1.0, palette.glass_stroke), egui::StrokeKind::Inside);
    ui.painter().text(rect.left_center() + vec2(22.0, 0.0), Align2::LEFT_CENTER, icon::MAGNIFYING_GLASS, theme::icon_font(20.0), palette.secondary);

    let edit_rect = Rect::from_min_max(rect.min + vec2(52.0, 8.0), rect.max - vec2(52.0, 8.0));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(edit_rect));
    let response = child.add(
        egui::TextEdit::singleline(&mut state.query)
            .hint_text(t("Titles, authors..."))
            .frame(egui::Frame::NONE)
            .font(theme::font(17.0))
            .desired_width(edit_rect.width())
            .vertical_align(egui::Align::Center)
            .min_size(vec2(edit_rect.width(), edit_rect.height())),
    );
    if response.changed() {
        state.changed_at = Some(Instant::now());
    }
    if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
        shared.store.remember_search(&state.query);
        state.changed_at = None;
    }
    if ui.input(|i| i.modifiers.command && i.key_pressed(egui::Key::F)) {
        response.request_focus();
    }

    if !state.query.is_empty() {
        let clear = Rect::from_center_size(rect.right_center() - vec2(28.0, 0.0), vec2(30.0, 30.0));
        let clear_response = ui.interact(clear, ui.id().with("clear-search"), Sense::click());
        ui.painter().text(clear.center(), Align2::CENTER_CENTER, icon::X_CIRCLE, theme::icon_font(20.0), palette.secondary);
        if clear_response.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
            state.query.clear();
            response.request_focus();
        }
    }
}

fn results(ui: &mut Ui, state: &mut SearchState, shared: &mut Shared, adult: bool) {
    if !net::is_online() {
        widgets::empty_state(ui, icon::WIFI_SLASH, t("You're offline"), t("Search needs a connection."));
        return;
    }
    let loading_first = state.request.as_ref().map(|r| !r.1).unwrap_or(false);
    if state.results.is_empty() {
        if loading_first || state.key != state.wanted_key(adult) {
            widgets::spinner(ui);
        } else if let Some(error) = state.error.clone() {
            if widgets::error_state(ui, &error) {
                state.key.clear();
            }
        } else {
            let what = if state.trimmed().is_empty() { state.tag.clone().unwrap_or_default() } else { state.trimmed() };
            widgets::empty_state(ui, icon::MAGNIFYING_GLASS, &tf("No results for \"{what}\"", &[("what", &what)]), t("Check the spelling or try a new search."));
        }
        return;
    }

    let items = state.results.clone();
    let last = items.len() - 1;
    let mut reached_end = false;
    for (index, manga) in items.iter().enumerate() {
        let response = widgets::manga_row(ui, &mut shared.images, manga, None);
        if response.clicked() {
            shared.store.remember_search(&state.query);
            shared.router.open(manga);
        }
        if index == last && ui.is_rect_visible(response.rect) {
            reached_end = true;
        }
        ui.add_space(4.0);
    }
    if reached_end && state.has_more && state.request.is_none() {
        state.start(ui, adult, true);
    }
    if state.request.is_some() {
        widgets::spinner(ui);
    }
}

fn discover(ui: &mut Ui, state: &mut SearchState, shared: &mut Shared, palette: &Palette) {
    if !net::is_online() {
        widgets::offline_banner(ui, t("You're offline. Search needs a connection."));
        ui.add_space(18.0);
    }

    let recent = shared.store.settings.recent_searches.clone();
    if !recent.is_empty() {
        ui.horizontal(|ui| {
            ui.label(RichText::new(t("Recent")).font(theme::bold_font(20.0)));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if widgets::link_button(ui, t("Clear")).clicked() {
                    shared.store.settings.recent_searches.clear();
                    shared.store.settings_changed();
                }
            });
        });
        ui.horizontal_wrapped(|ui| {
            for term in &recent {
                if widgets::pill(ui, term, Some(icon::CLOCK_COUNTER_CLOCKWISE), false, 0.0).clicked() {
                    state.query = term.clone();
                    state.changed_at = None;
                }
            }
        });
        ui.add_space(24.0);
    }

    ui.label(RichText::new(t("Genres")).font(theme::bold_font(20.0)));
    ui.add_space(8.0);
    grid(ui, GENRES.len(), 210.0, 14.0, |ui, index, width| {
        let (name, glyph) = GENRES[index];
        let hue = index as f32 / GENRES.len() as f32;
        if genre_tile(ui, t(name), glyph, hue, width).clicked() {
            state.tag = Some(name.to_string());
        }
    });
    let _ = palette;
}

fn genre_tile(ui: &mut Ui, name: &str, glyph: &str, hue: f32, width: f32) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(vec2(width, 92.0), Sense::click());
    // HsvaGamma values match what is on screen, plain Hsva is linear and washes out
    let top = egui::ecolor::HsvaGamma { h: hue, s: 0.62, v: 0.80, a: 1.0 };
    let bottom = egui::ecolor::HsvaGamma { h: (hue + 0.06) % 1.0, s: 0.72, v: 0.52, a: 1.0 };
    let hover = ui.ctx().animate_bool_with_time(response.id.with("h"), response.hovered(), 0.15);
    let painter = ui.painter_at(rect);
    theme::rounded_gradient(&painter, rect, 18.0, top.into(), bottom.into());
    if hover > 0.0 {
        painter.rect_filled(rect, CornerRadius::same(18), Color32::from_white_alpha((22.0 * hover) as u8));
    }
    painter.text(rect.right_top() + vec2(-14.0, 6.0), Align2::RIGHT_TOP, glyph, theme::icon_font(54.0), Color32::from_white_alpha(70));
    painter.text(rect.left_bottom() + vec2(16.0, -16.0), Align2::LEFT_BOTTOM, format!("{glyph}  {name}"), theme::bold_font(16.5), Color32::WHITE);
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}
