// full-window reader: vertical or paged (ltr / rtl), tap zones and keyboard shortcuts

use std::collections::HashMap;
use std::time::{Duration, Instant};

use egui::{Align2, Color32, CornerRadius, Key, PointerButton, Rect, RichText, Sense, Stroke, StrokeKind, Ui, pos2, vec2};
use crate::icons as icon;
use poll_promise::Promise;

use super::widgets;
use crate::app::{ReaderRequest, Shared, spawn};
use crate::images::{Kind, Loaded, State};
use crate::model::Chapter;
use crate::net;
use crate::source;
use crate::store::{ReaderBackground, ReaderMode, TwoPages, catalog};
use crate::theme::{self, Palette};
use crate::i18n::{t, tf};

// the controls and the cursor hide after this long without mouse movement
const IDLE: Duration = Duration::from_millis(2500);
// moving the mouse this close to the top or bottom edge brings the controls up
const TOP_EDGE: f32 = 84.0;
const BOTTOM_EDGE: f32 = 110.0;
// arrow keys held down in vertical mode, pixels per second
const HOLD_SPEED: f32 = 1150.0;

enum Phase {
    Loading,
    Loaded,
    Failed(String),
}

pub struct ReaderState {
    origin: String,
    manga_id: String,
    manga_title: String,
    chapters: Vec<Chapter>,
    chapter: Chapter,
    pages: Vec<String>,
    phase: Phase,
    // pages.len() is the end of chapter page
    position: usize,
    reached_end: bool,
    show_chrome: bool,
    settings_open: bool,
    chapters_open: bool,
    request: Option<Promise<source::Result<Vec<String>>>>,
    chapter_list: Option<Promise<source::Result<Vec<Chapter>>>>,
    // remembered page shapes so the vertical layout doesn't jump
    aspects: HashMap<String, f32>,
    jump_to: Option<usize>,
    scroll_by: f32,
    zoom: f32,
    pan: egui::Vec2,
    last_saved: Option<usize>,
    // two pages side by side right now, and the pages on screen
    double: bool,
    spread: Option<Spread>,
    last_motion: Instant,
    // the last page change, for the slide and the page counter. turn_dir is 0 when scrolling.
    turned_at: Option<Instant>,
    turn_dir: f32,
    // vertical mode: the page at the top of the window and how far into it.
    // when pages above change size (a page loads, the width changes) the view stays on it.
    anchor: Option<(usize, f32)>,
    scroll_offset: f32,
    // page lists of the next chapter, fetched near the end so it opens at once
    prefetched: HashMap<String, Vec<String>>,
    prefetch: Option<(String, Promise<source::Result<Vec<String>>>)>,
    // chapters already asked for, so a failed fetch isn't repeated on every frame
    prefetch_tried: std::collections::HashSet<String>,
}

impl ReaderState {
    pub fn new(request: ReaderRequest, ctx: &egui::Context, shared: &Shared) -> Self {
        let mut chapters = request.chapters.clone();
        if chapters.is_empty() {
            chapters = catalog::chapters(&request.manga_id);
        }
        let mut state = Self {
            origin: request.chapter.id.clone(),
            manga_id: request.manga_id.clone(),
            manga_title: request.manga_title.clone(),
            chapters,
            chapter: request.chapter.clone(),
            pages: Vec::new(),
            phase: Phase::Loading,
            position: 0,
            reached_end: false,
            show_chrome: false,
            settings_open: false,
            chapters_open: false,
            request: None,
            chapter_list: None,
            aspects: HashMap::new(),
            jump_to: None,
            scroll_by: 0.0,
            zoom: 1.0,
            pan: egui::Vec2::ZERO,
            last_saved: None,
            double: false,
            spread: None,
            last_motion: Instant::now(),
            turned_at: None,
            turn_dir: 0.0,
            anchor: None,
            scroll_offset: 0.0,
            prefetched: HashMap::new(),
            prefetch: None,
            prefetch_tried: Default::default(),
        };
        // developer switch for screenshots, e.g. YOMIKAZE_READER=chrome,chapters,settings
        if let Ok(open) = std::env::var("YOMIKAZE_READER") {
            state.show_chrome = open.contains("chrome");
            state.chapters_open = open.contains("chapters");
            state.settings_open = open.contains("settings");
        }
        if state.chapters.is_empty() && net::is_online() {
            let id = state.manga_id.clone();
            state.chapter_list = Some(spawn(ctx, move || source::chapters(&id)));
        }
        state.load(ctx, shared);
        state
    }

    pub fn owns(&self, request: &ReaderRequest) -> bool {
        self.origin == request.chapter.id && self.manga_id == request.manga_id
    }

    fn load(&mut self, ctx: &egui::Context, shared: &Shared) {
        self.phase = Phase::Loading;
        self.pages.clear();
        self.position = 0;
        self.reached_end = false;
        self.zoom = 1.0;
        self.pan = egui::Vec2::ZERO;
        self.last_saved = None;
        self.anchor = None;
        self.scroll_offset = 0.0;
        self.turned_at = None;
        if let Some(download) = shared.store.downloads.get(&self.chapter.id) {
            let pages = download.page_urls.clone();
            self.request = Some(spawn(ctx, move || Ok(pages)));
        } else if let Some(pages) = self.prefetched.remove(&self.chapter.id) {
            self.request = Some(spawn(ctx, move || Ok(pages)));
        } else if !net::is_online() {
            self.phase = Phase::Failed(t("You're offline and this chapter isn't downloaded.").to_string());
        } else {
            let id = self.chapter.id.clone();
            self.request = Some(spawn(ctx, move || source::pages(&id)));
        }
    }

    fn poll(&mut self, shared: &Shared) {
        if let Some((id, promise)) = &self.prefetch {
            if let Some(result) = promise.ready() {
                if let Ok(pages) = result {
                    self.prefetched.insert(id.clone(), pages.clone());
                }
                self.prefetch = None;
            }
        }
        if let Some(promise) = &self.chapter_list {
            if let Some(result) = promise.ready() {
                if let Ok(list) = result {
                    if !list.is_empty() {
                        catalog::save(&catalog::chapters_name(&self.manga_id), list);
                        self.chapters = list.clone();
                    }
                }
                self.chapter_list = None;
            }
        }
        let Some(promise) = &self.request else { return };
        let Some(result) = promise.ready() else { return };
        match result {
            Ok(pages) if !pages.is_empty() => {
                self.pages = pages.clone();
                let start = shared
                    .store
                    .progress
                    .get(&self.chapter.id)
                    .filter(|p| !p.completed)
                    .map(|p| p.page.min(self.pages.len() - 1))
                    .unwrap_or(0);
                self.position = start;
                self.jump_to = Some(start);
                self.phase = Phase::Loaded;
            }
            Ok(_) => self.phase = Phase::Failed(t("This chapter has no pages.").to_string()),
            Err(error) => self.phase = Phase::Failed(error.clone()),
        }
        self.request = None;
    }

    // close to the end, fetch the next chapter's page list and its first pages
    fn prepare_next(&mut self, ctx: &egui::Context, shared: &mut Shared) {
        if self.position + 4 < self.pages.len() {
            return;
        }
        let Some(next) = self.next_chapter() else { return };
        if let Some(pages) = self.prefetched.get(&next.id) {
            for url in pages.iter().take(2) {
                shared.images.prefetch(url, Kind::Page);
            }
            return;
        }
        if self.prefetch.is_some() || shared.store.downloads.contains_key(&next.id) || !net::is_online() {
            return;
        }
        if !self.prefetch_tried.insert(next.id.clone()) {
            return;
        }
        let id = next.id.clone();
        self.prefetch = Some((next.id, spawn(ctx, move || source::pages(&id))));
    }

    fn index(&self) -> Option<usize> {
        self.chapters.iter().position(|c| c.id == self.chapter.id)
    }

    fn next_chapter(&self) -> Option<Chapter> {
        self.index().filter(|&i| i > 0).map(|i| self.chapters[i - 1].clone())
    }

    fn previous_chapter(&self) -> Option<Chapter> {
        self.index().filter(|&i| i + 1 < self.chapters.len()).map(|i| self.chapters[i + 1].clone())
    }

    fn save_progress(&mut self, shared: &mut Shared) {
        if !matches!(self.phase, Phase::Loaded) || self.pages.is_empty() {
            return;
        }
        let page = self.position.min(self.pages.len() - 1);
        let finished = self.reached_end || self.position + 1 >= self.pages.len();
        if self.last_saved == Some(self.position) && !finished {
            return;
        }
        self.last_saved = Some(self.position);
        shared.store.record_progress(&self.manga_id, &self.manga_title, &self.chapter, page, self.pages.len(), finished);
    }

    fn go_to(&mut self, chapter: Chapter, ctx: &egui::Context, shared: &mut Shared) {
        self.save_progress(shared);
        self.chapter = chapter;
        self.load(ctx, shared);
    }

    pub fn close(&mut self, shared: &mut Shared) {
        self.save_progress(shared);
        shared.store.flush(true);
    }

    fn idle(&self) -> bool {
        self.last_motion.elapsed() > IDLE
    }
}

struct Keys {
    escape: bool,
    left: bool,
    right: bool,
    page_down: bool,
    page_up: bool,
    arrow_down: bool,
    arrow_up: bool,
    space: bool,
    shift: bool,
    fullscreen: bool,
    home: bool,
    end: bool,
    back: bool,
    forward: bool,
    // -1, 0 or 1 while an up or down key is held
    hold: f32,
}

fn read_keys(ctx: &egui::Context) -> Keys {
    ctx.input(|i| Keys {
        escape: i.key_pressed(Key::Escape),
        left: i.key_pressed(Key::ArrowLeft) || i.key_pressed(Key::A),
        right: i.key_pressed(Key::ArrowRight) || i.key_pressed(Key::D),
        page_down: i.key_pressed(Key::PageDown),
        page_up: i.key_pressed(Key::PageUp),
        arrow_down: i.key_pressed(Key::ArrowDown) || i.key_pressed(Key::S),
        arrow_up: i.key_pressed(Key::ArrowUp) || i.key_pressed(Key::W),
        space: i.key_pressed(Key::Space),
        shift: i.modifiers.shift,
        fullscreen: i.key_pressed(Key::F11) || i.key_pressed(Key::F),
        home: i.key_pressed(Key::Home),
        end: i.key_pressed(Key::End),
        back: i.pointer.button_pressed(PointerButton::Extra1),
        forward: i.pointer.button_pressed(PointerButton::Extra2),
        hold: (i.key_down(Key::ArrowDown) || i.key_down(Key::S)) as i32 as f32 - (i.key_down(Key::ArrowUp) || i.key_down(Key::W)) as i32 as f32,
    })
}

// returns false when the reader should close
pub fn show(ui: &mut Ui, state: &mut ReaderState, shared: &mut Shared) -> bool {
    let ctx = ui.ctx().clone();
    state.poll(shared);
    let settings = shared.store.settings.clone();
    let rect = ui.max_rect();
    ui.painter().rect_filled(rect, 0.0, settings.reader_background.color());

    let mut keep_open = true;
    let loaded = matches!(state.phase, Phase::Loaded);

    let moved = ctx.input(|i| i.pointer.delta().length_sq() > 0.0 || i.pointer.any_pressed());
    if moved {
        state.last_motion = Instant::now();
    }

    let paged = settings.reader_mode != ReaderMode::Vertical;
    let rtl = settings.reader_mode == ReaderMode::PagedRtl;
    let keys = read_keys(&ctx);
    let no_dialog = !state.settings_open;
    if no_dialog && ctx.input(|i| i.key_pressed(Key::Tab)) {
        state.show_chrome = !state.show_chrome;
    }
    if no_dialog && loaded && ctx.input(|i| i.key_pressed(Key::C)) {
        state.chapters_open = !state.chapters_open;
    }
    if paged && no_dialog && ctx.input(|i| i.key_pressed(Key::T)) {
        toggle_two_pages(state, shared);
    }
    if keys.escape && no_dialog {
        if state.chapters_open {
            state.chapters_open = false;
        } else if ctx.input(|i| i.viewport().fullscreen.unwrap_or(false)) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
        } else {
            keep_open = false;
        }
    }
    if keys.fullscreen && no_dialog {
        toggle_fullscreen(&ctx);
    }
    let mut hold = 0.0;
    if loaded && no_dialog {
        let forward = if rtl { keys.left } else { keys.right };
        let backward = if rtl { keys.right } else { keys.left };
        if paged {
            if forward || keys.page_down || keys.arrow_down || (keys.space && !keys.shift) || keys.forward {
                turn(state, 1);
            }
            if backward || keys.page_up || keys.arrow_up || (keys.space && keys.shift) || keys.back {
                turn(state, -1);
            }
            if keys.home {
                state.position = 0;
            }
            if keys.end {
                state.position = state.pages.len().saturating_sub(1);
            }
        } else {
            let step = rect.height() * 0.8;
            if keys.page_down || (keys.space && !keys.shift) || keys.right || keys.forward {
                state.scroll_by += step;
            }
            if keys.page_up || (keys.space && keys.shift) || keys.left || keys.back {
                state.scroll_by -= step;
            }
            hold = keys.hold;
            if keys.home {
                state.jump_to = Some(0);
            }
            if keys.end {
                state.jump_to = Some(state.pages.len());
            }
        }
    }

    match &state.phase {
        Phase::Loading => loading(ui, rect, state),
        Phase::Failed(message) => {
            let message = message.clone();
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_center_size(rect.center(), vec2(440.0, 320.0))));
            child.style_mut().visuals.override_text_color = Some(Color32::WHITE);
            if widgets::error_state(&mut child, &message) {
                state.load(&ctx, shared);
            }
            child.vertical_centered(|ui| {
                ui.add_space(6.0);
                if widgets::glass_button(ui, t("Close"), None, Some(140.0)).clicked() {
                    keep_open = false;
                }
            });
        }
        Phase::Loaded => {
            if paged {
                paged_reader(ui, rect, state, shared, rtl, &settings, &mut keep_open);
            } else {
                vertical_reader(ui, rect, state, shared, &settings, hold, &mut keep_open);
            }
        }
    }

    // software dimming, darker than the lowest screen brightness
    if settings.reader_dim > 0.0 {
        ui.painter().rect_filled(rect, 0.0, Color32::from_black_alpha((255.0 * settings.reader_dim) as u8));
    }

    if matches!(state.phase, Phase::Loaded) {
        for url in state.pages.iter().skip(state.position + 1).take(5) {
            shared.images.prefetch(url, Kind::Page);
        }
        state.prepare_next(&ctx, shared);
    }

    state.save_progress(shared);
    // the controls make room for the chapter list instead of sitting under it
    let drawer = ctx.animate_bool_with_time(egui::Id::new("reader-chapters-drawer"), state.chapters_open, 0.22);
    let free = Rect::from_min_max(rect.min, pos2(rect.right() - drawer_width(rect) * theme::ease_out_cubic(drawer), rect.bottom()));
    let shown = chrome(ui, free, state, shared, &settings, &mut keep_open);
    page_feedback(ui, rect, state, &settings, shown);
    chapters_drawer(ui, rect, state, shared, drawer);
    settings_sheet(ui, state, shared);

    // the cursor gets out of the way while reading
    let busy = state.settings_open || state.chapters_open || !matches!(state.phase, Phase::Loaded);
    if state.idle() {
        if !busy && shown <= 0.0 && ui.rect_contains_pointer(rect) {
            ctx.set_cursor_icon(egui::CursorIcon::None);
        }
    } else {
        ctx.request_repaint_after(IDLE + Duration::from_millis(50));
    }
    keep_open
}

fn toggle_two_pages(state: &ReaderState, shared: &mut Shared) {
    shared.store.settings.two_pages = if state.double { TwoPages::Off } else { TwoPages::On };
    shared.store.settings_changed();
}

fn toggle_fullscreen(ctx: &egui::Context) {
    let fullscreen = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
    ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(!fullscreen));
}

fn loading(ui: &mut Ui, rect: Rect, state: &ReaderState) {
    let ctx = ui.ctx().clone();
    let palette = Palette::of(&ctx);
    let center = rect.center();
    // three dots breathing one after another
    let time = ctx.input(|i| i.time) as f32;
    for k in 0..3 {
        let phase = (time * 3.2 - k as f32 * 0.55).sin() * 0.5 + 0.5;
        let dot = pos2(center.x + (k as f32 - 1.0) * 18.0, center.y - 4.0 * phase);
        ui.painter().circle_filled(dot, 4.5, palette.accent.gamma_multiply(0.35 + 0.65 * phase));
    }
    let label = format!("{} · {}", state.manga_title, state.chapter.name);
    ui.painter().text(center + vec2(0.0, 34.0), Align2::CENTER_CENTER, label, theme::font(13.0), Color32::from_white_alpha(120));
    ctx.request_repaint();
}

// the pages on screen in two-page mode. turning goes on from the last page shown (or back from
// the first one), so a page never shows up in two spreads in a row, even when the shape of a
// page only becomes known after the spreads around it were already on screen.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Spread {
    first: usize,
    second: Option<usize>,
    // made by turning back: if the pair falls apart, the later page is the one to keep
    from_end: bool,
}

impl Spread {
    fn last(&self) -> usize {
        self.second.unwrap_or(self.first)
    }
}

// two pages share the screen unless one is the cover or a wide two-page drawing
fn pairs(first: usize, second: usize, count: usize, wide: &impl Fn(usize) -> bool) -> bool {
    first >= 1 && second == first + 1 && second < count && !wide(first) && !wide(second)
}

fn spread_from(start: usize, count: usize, wide: &impl Fn(usize) -> bool) -> Spread {
    let second = pairs(start, start + 1, count, wide).then_some(start + 1);
    Spread { first: start, second, from_end: false }
}

fn spread_to(end: usize, count: usize, wide: &impl Fn(usize) -> bool) -> Spread {
    if end >= 1 && pairs(end - 1, end, count, wide) {
        Spread { first: end - 1, second: Some(end), from_end: true }
    } else {
        Spread { first: end, second: None, from_end: true }
    }
}

// a spread stays as it was turned to, unless one of its pages turned out to be wide
fn settle(spread: Spread, count: usize, wide: &impl Fn(usize) -> bool) -> Spread {
    match spread.second {
        Some(second) if !pairs(spread.first, second, count, wide) => {
            let keep = if spread.from_end { second } else { spread.first };
            Spread { first: keep, second: None, from_end: false }
        }
        _ => spread,
    }
}

// the next or previous spread. None is the end of chapter screen after the last page.
fn step(current: Option<Spread>, delta: i32, count: usize, wide: &impl Fn(usize) -> bool) -> Option<Spread> {
    if count == 0 {
        return None;
    }
    if delta > 0 {
        let next = current?.last() + 1;
        (next < count).then(|| spread_from(next, count, wide))
    } else {
        Some(match current {
            None => spread_to(count - 1, count, wide),
            Some(spread) if spread.first == 0 => spread,
            Some(spread) => spread_to(spread.first - 1, count, wide),
        })
    }
}

// a page is wide (a two-page drawing) once its shape is known
fn is_wide(state: &ReaderState, index: usize) -> bool {
    state.pages.get(index).and_then(|url| state.aspects.get(url)).is_some_and(|aspect| *aspect > 1.05)
}

fn turn(state: &mut ReaderState, delta: i32) {
    let before = state.position;
    if state.double && !state.pages.is_empty() {
        let count = state.pages.len();
        let next = {
            let wide = |index: usize| is_wide(state, index);
            let current = (state.position < count).then(|| {
                state.spread.filter(|s| s.first == state.position).unwrap_or_else(|| spread_from(state.position, count, &wide))
            });
            step(current, delta, count, &wide)
        };
        state.position = next.map_or(count, |s| s.first);
        state.spread = next;
    } else {
        let target = (state.position as i32 + delta).clamp(0, state.pages.len() as i32);
        state.position = target as usize;
    }
    if state.position != before {
        state.zoom = 1.0;
        state.pan = egui::Vec2::ZERO;
        state.turned_at = Some(Instant::now());
        state.turn_dir = delta.signum() as f32;
    }
    if state.position >= state.pages.len() {
        state.reached_end = true;
    }
}

// slide offset and opacity of a page that was just turned to
fn turn_motion(state: &ReaderState, enabled: bool, rtl: bool, ctx: &egui::Context) -> (f32, f32) {
    let Some(at) = state.turned_at else { return (0.0, 1.0) };
    if !enabled || state.turn_dir == 0.0 {
        return (0.0, 1.0);
    }
    let t = (at.elapsed().as_secs_f32() / 0.22).min(1.0);
    if t >= 1.0 {
        return (0.0, 1.0);
    }
    ctx.request_repaint();
    let ease = theme::ease_out_cubic(t);
    let direction = if rtl { -state.turn_dir } else { state.turn_dir };
    (direction * 46.0 * (1.0 - ease), 0.2 + 0.8 * ease)
}

// edges turn the page, the middle shows or hides the controls
fn handle_tap(state: &mut ReaderState, x: f32, rect: Rect, tap_zones: bool, paged: bool, rtl: bool) {
    if state.chapters_open {
        state.chapters_open = false;
        return;
    }
    let fraction = (x - rect.left()) / rect.width();
    if !tap_zones || (0.28..=0.72).contains(&fraction) {
        state.show_chrome = !state.show_chrome;
        return;
    }
    let forward = if rtl { -1 } else { 1 };
    let direction = if fraction < 0.28 { -forward } else { forward };
    if paged {
        turn(state, direction);
    } else {
        state.scroll_by += direction as f32 * rect.height() * 0.8;
    }
}

fn vertical_reader(ui: &mut Ui, rect: Rect, state: &mut ReaderState, shared: &mut Shared, settings: &crate::store::Settings, hold: f32, keep_open: &mut bool) {
    // ctrl + wheel makes the pages wider or narrower
    let zoom_delta = ui.input(|i| i.zoom_delta());
    if zoom_delta != 1.0 && ui.rect_contains_pointer(rect) {
        let width = (shared.store.settings.reader_width * zoom_delta).clamp(360.0, 2400.0);
        shared.store.settings.reader_width = width;
        shared.store.settings_changed();
    }
    let width = shared.store.settings.reader_width.min(rect.width());
    let gap = if settings.reader_gap { 14.0 } else { 0.0 };
    let left = rect.center().x - width / 2.0;

    // 0.7 width/height until a page has loaded
    let heights: Vec<f32> = state
        .pages
        .iter()
        .map(|url| width / state.aspects.get(url).copied().unwrap_or(0.7))
        .collect();
    let mut tops = Vec::with_capacity(heights.len() + 1);
    let mut y = 0.0;
    for h in &heights {
        tops.push(y);
        y += h + gap;
    }
    tops.push(y);
    let end_height = 420.0;
    let total = y + end_height;

    let mut scroll = egui::ScrollArea::vertical()
        .id_salt(("reader", &state.chapter.id))
        .auto_shrink([false, false])
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden);
    if let Some(page) = state.jump_to.take() {
        scroll = scroll.vertical_scroll_offset(tops[page.min(tops.len() - 1)]);
    } else if let Some((page, within)) = state.anchor.filter(|(page, _)| *page < heights.len()) {
        let wanted = tops[page] + within * heights[page];
        if (wanted - state.scroll_offset).abs() > 1.0 {
            scroll = scroll.vertical_scroll_offset(wanted);
        }
    }
    // registered before the pages so the end of chapter buttons stay clickable
    let taps = ui.interact(rect, ui.id().with(("taps", &state.chapter.id)), Sense::click());
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect));
    let scroll_by = std::mem::take(&mut state.scroll_by);
    let output = scroll.show_viewport(&mut child, |ui, viewport| {
        ui.set_min_height(total);
        ui.set_min_width(rect.width());
        if scroll_by != 0.0 {
            ui.scroll_with_delta_animation(vec2(0.0, -scroll_by), egui::style::ScrollAnimation::duration(0.25));
        }
        if hold != 0.0 {
            let dt = ui.input(|i| i.stable_dt).min(0.05);
            ui.scroll_with_delta(vec2(0.0, -hold * HOLD_SPEED * dt));
            ui.ctx().request_repaint();
        }
        let origin = ui.min_rect().min;
        let mut current = 0;
        for (index, url) in state.pages.iter().enumerate() {
            let top = tops[index];
            let height = heights[index];
            if top + height < viewport.min.y - 1200.0 || top > viewport.max.y + 1200.0 {
                continue;
            }
            if top <= viewport.min.y + viewport.height() * 0.3 {
                current = index;
            }
            let page_rect = Rect::from_min_size(pos2(left, origin.y + top), vec2(width, height));
            match shared.images.get(url, Kind::Page) {
                State::Ready(loaded) => {
                    state.aspects.insert(url.clone(), loaded.aspect());
                    paint_tiles(ui, loaded, page_rect);
                }
                State::Loading => {
                    page_placeholder(ui, page_rect, index, false);
                }
                State::Failed => {
                    if page_placeholder(ui, page_rect, index, true) {
                        shared.images.retry(url, Kind::Page);
                    }
                }
            }
        }
        if !heights.is_empty() {
            let page = tops.partition_point(|t| *t <= viewport.min.y).saturating_sub(1).min(heights.len() - 1);
            let within = ((viewport.min.y - tops[page]) / heights[page].max(1.0)).clamp(0.0, 1.0);
            state.anchor = Some((page, within));
        }
        let end_rect = Rect::from_min_size(pos2(rect.left(), origin.y + tops[state.pages.len()]), vec2(rect.width(), end_height));
        if end_rect.top() < origin.y + viewport.max.y {
            if viewport.max.y > tops[state.pages.len()] + 120.0 {
                state.reached_end = true;
                current = state.pages.len();
            }
            chapter_end(ui, end_rect, state, shared, keep_open);
        }
        current
    });
    state.scroll_offset = output.state.offset.y;
    let current = output.inner;
    if current != state.position {
        state.position = current;
        state.turned_at = Some(Instant::now());
        state.turn_dir = 0.0;
    }

    if taps.clicked() {
        if let Some(pos) = taps.interact_pointer_pos() {
            let on_end = state.position >= state.pages.len();
            if !on_end {
                handle_tap(state, pos.x, rect, settings.tap_zones, false, false);
            }
        }
    }
}

fn paint_tiles(ui: &Ui, loaded: &Loaded, rect: Rect) {
    draw_loaded(ui.painter(), loaded, rect, Color32::WHITE);
}

fn page_placeholder(ui: &mut Ui, rect: Rect, index: usize, failed: bool) -> bool {
    let center = rect.center();
    if failed {
        ui.painter().text(center - vec2(0.0, 24.0), Align2::CENTER_CENTER, icon::WARNING, theme::icon_font(26.0), Color32::from_white_alpha(200));
        ui.painter().text(center, Align2::CENTER_CENTER, tf("Page {n} couldn't be loaded", &[("n", &(index + 1))]), theme::font(14.0), Color32::from_white_alpha(200));
        let button = Rect::from_center_size(center + vec2(0.0, 40.0), vec2(110.0, 36.0));
        let response = ui.interact(button, ui.id().with(("retry", index)), Sense::click());
        ui.painter().rect_filled(button, CornerRadius::same(255), Color32::from_white_alpha(40));
        ui.painter().text(button.center(), Align2::CENTER_CENTER, t("Retry"), theme::bold_font(14.0), Color32::WHITE);
        response.clicked()
    } else {
        // a soft pulse instead of a spinner on every page
        let time = ui.input(|i| i.time) as f32;
        let pulse = (time * 2.4 + index as f32).sin() * 0.5 + 0.5;
        let block = Rect::from_center_size(center, vec2(rect.width().min(220.0), rect.height().min(300.0)) * 0.6);
        ui.painter().rect_filled(block, CornerRadius::same(14), Color32::from_white_alpha((8.0 + 10.0 * pulse) as u8));
        ui.painter().text(center, Align2::CENTER_CENTER, (index + 1).to_string(), theme::bold_font(15.0), Color32::from_white_alpha(110));
        ui.ctx().request_repaint();
        false
    }
}

// width / height of a page, remembered once its image has loaded
fn page_aspect(state: &mut ReaderState, shared: &mut Shared, index: usize) -> Option<f32> {
    let url = state.pages.get(index)?.clone();
    if let Some(aspect) = state.aspects.get(&url) {
        return Some(*aspect);
    }
    if let State::Ready(loaded) = shared.images.get(&url, Kind::Page) {
        let aspect = loaded.size.x / loaded.size.y.max(1.0);
        state.aspects.insert(url, aspect);
        return Some(aspect);
    }
    None
}

fn draw_loaded(painter: &egui::Painter, loaded: &Loaded, page_rect: Rect, tint: Color32) {
    let scale = page_rect.width() / loaded.size.x.max(1.0);
    let mut y = page_rect.top();
    for (texture, tile_size) in &loaded.tiles {
        let tile = Rect::from_min_size(pos2(page_rect.left(), y), vec2(page_rect.width(), tile_size.y * scale));
        painter.image(texture.id(), tile, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), tint);
        y += tile_size.y * scale;
    }
}

fn paged_reader(ui: &mut Ui, rect: Rect, state: &mut ReaderState, shared: &mut Shared, rtl: bool, settings: &crate::store::Settings, keep_open: &mut bool) {
    let response = ui.interact(rect, ui.id().with(("paged", &state.chapter.id)), Sense::click_and_drag());

    state.double = match settings.two_pages {
        TwoPages::On => true,
        TwoPages::Off => false,
        TwoPages::Auto => rect.width() >= 1000.0 && rect.width() > rect.height() * 1.2,
    };
    if state.double && state.position < state.pages.len() {
        // look at the pages around the current one so wide pages are known before they pair up
        let count = state.pages.len();
        for index in state.position.saturating_sub(2)..(state.position + 3).min(count) {
            page_aspect(state, shared, index);
        }
        let shown = {
            let wide = |index: usize| is_wide(state, index);
            match state.spread {
                Some(spread) if spread.first == state.position => settle(spread, count, &wide),
                // a jump (slider, home, end, a new chapter) starts a fresh spread
                _ => spread_from(state.position, count, &wide),
            }
        };
        state.position = shown.first;
        state.spread = Some(shown);
    } else {
        state.spread = None;
    }
    let spread = state.spread.map(|s| (s.first, s.second));

    if state.position >= state.pages.len() {
        state.reached_end = true;
        chapter_end(ui, rect, state, shared, keep_open);
    } else {
        let url = state.pages[state.position].clone();
        // ctrl + wheel or pinch zooms, drag pans, double click toggles 2x
        let (zoom_delta, pointer) = ui.input(|i| (i.zoom_delta(), i.pointer.hover_pos()));
        if zoom_delta != 1.0 && response.hovered() {
            let old = state.zoom;
            state.zoom = (state.zoom * zoom_delta).clamp(1.0, 4.0);
            if let Some(p) = pointer {
                let anchor = p - rect.center() - state.pan;
                state.pan -= anchor * (state.zoom / old - 1.0);
            }
        }
        if response.double_clicked() {
            if state.zoom > 1.01 {
                state.zoom = 1.0;
                state.pan = egui::Vec2::ZERO;
            } else {
                state.zoom = 2.0;
                if let Some(p) = response.interact_pointer_pos() {
                    state.pan = -(p - rect.center());
                }
            }
        }
        if state.zoom > 1.0 && response.dragged() {
            state.pan += response.drag_delta();
        }
        if state.zoom <= 1.0 {
            state.pan = egui::Vec2::ZERO;
        }

        let (slide, alpha) = turn_motion(state, settings.page_animation, rtl, ui.ctx());
        let shift = vec2(slide, 0.0);
        let tint = Color32::WHITE.gamma_multiply(alpha);
        let painter = ui.painter_at(rect);
        if let Some((first, Some(second))) = spread {
            // the second page goes on the left in right-to-left mode, like a printed manga
            let (left, right) = if rtl { (second, first) } else { (first, second) };
            let left_aspect = page_aspect(state, shared, left).unwrap_or(0.7);
            let right_aspect = page_aspect(state, shared, right).unwrap_or(0.7);
            let height = rect.height().min(rect.width() / (left_aspect + right_aspect)) * state.zoom;
            let group = Rect::from_center_size(rect.center() + state.pan + shift, vec2(height * (left_aspect + right_aspect), height));
            let left_rect = Rect::from_min_size(group.min, vec2(height * left_aspect, height));
            let right_rect = Rect::from_min_size(pos2(left_rect.right(), group.top()), vec2(height * right_aspect, height));
            for (index, page_rect) in [(left, left_rect), (right, right_rect)] {
                let page_url = state.pages[index].clone();
                match shared.images.get(&page_url, Kind::Page) {
                    State::Ready(loaded) => draw_loaded(&painter, loaded, page_rect, tint),
                    State::Loading => {
                        page_placeholder(ui, page_rect.shrink(20.0), index, false);
                    }
                    State::Failed => {
                        if page_placeholder(ui, page_rect.shrink(20.0), index, true) {
                            shared.images.retry(&page_url, Kind::Page);
                        }
                    }
                }
            }
            // a faint fold between the two pages
            let fold = Rect::from_center_size(pos2(left_rect.right(), group.center().y), vec2(18.0, group.height()));
            let shade = Color32::from_black_alpha((26.0 * alpha) as u8);
            theme::horizontal_gradient(&painter, Rect::from_min_max(fold.min, pos2(fold.center().x, fold.max.y)), Color32::TRANSPARENT, shade);
            theme::horizontal_gradient(&painter, Rect::from_min_max(pos2(fold.center().x, fold.min.y), fold.max), shade, Color32::TRANSPARENT);
        } else {
            match shared.images.get(&url, Kind::Page) {
                State::Ready(loaded) => {
                    let fit = (rect.width() / loaded.size.x).min(rect.height() / loaded.size.y) * state.zoom;
                    let page_rect = Rect::from_center_size(rect.center() + state.pan + shift, loaded.size * fit);
                    draw_loaded(&painter, loaded, page_rect, tint);
                }
                State::Loading => {
                    page_placeholder(ui, Rect::from_center_size(rect.center(), vec2(300.0, 300.0)), state.position, false);
                }
                State::Failed => {
                    if page_placeholder(ui, Rect::from_center_size(rect.center(), vec2(300.0, 300.0)), state.position, true) {
                        shared.images.retry(&url, Kind::Page);
                    }
                }
            }
        }

        let wheel = ui.input(|i| if i.modifiers.ctrl { 0.0 } else { i.smooth_scroll_delta.y });
        if state.zoom <= 1.0 && response.hovered() && wheel.abs() > 0.0 {
            let accumulated = ui.ctx().data_mut(|d| {
                let value = d.get_temp_mut_or_default::<f32>(egui::Id::new("wheel-acc"));
                *value += wheel;
                *value
            });
            if accumulated.abs() > 60.0 {
                turn(state, if accumulated < 0.0 { 1 } else { -1 });
                ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("wheel-acc"), 0.0_f32));
            }
        }
    }

    if response.clicked() && state.position < state.pages.len() {
        if let Some(pos) = response.interact_pointer_pos() {
            handle_tap(state, pos.x, rect, settings.tap_zones, true, rtl);
        }
    }
}

fn chapter_end(ui: &mut Ui, rect: Rect, state: &mut ReaderState, shared: &mut Shared, keep_open: &mut bool) {
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_center_size(rect.center(), vec2(440.0, 320.0))));
    let palette = Palette::of(ui.ctx());
    let next = state.next_chapter();
    let mut go_next = false;
    let mut open_list = false;
    child.vertical_centered(|ui| {
        ui.label(RichText::new(icon::CHECK_CIRCLE).size(50.0).color(palette.accent));
        ui.add_space(2.0);
        ui.label(RichText::new(tf("End of {chapter}", &[("chapter", &state.chapter.name)])).font(theme::bold_font(19.0)).color(Color32::WHITE));
        ui.label(RichText::new(&state.manga_title).size(13.0).color(Color32::from_white_alpha(140)));
        ui.add_space(16.0);
        if let Some(next) = &next {
            if widgets::primary_button(ui, &tf("Next: {chapter}", &[("chapter", &next.name)]), Some(icon::CARET_CIRCLE_RIGHT), Some(220.0), true).clicked() {
                go_next = true;
            }
        } else {
            ui.label(RichText::new(t("You're all caught up.")).color(Color32::from_white_alpha(160)));
        }
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            let width = 130.0 * 2.0 + ui.spacing().item_spacing.x;
            ui.add_space(((ui.available_width() - width) / 2.0).max(0.0));
            if widgets::glass_button(ui, t("Chapters"), Some(icon::LIST), Some(130.0)).clicked() {
                open_list = true;
            }
            if widgets::glass_button(ui, t("Close"), None, Some(130.0)).clicked() {
                *keep_open = false;
            }
        });
    });
    if open_list {
        state.chapters_open = true;
    }
    if go_next {
        if let Some(next) = next {
            let ctx = ui.ctx().clone();
            state.go_to(next, &ctx, shared);
        }
    }
}

fn toggle_button(ui: &mut Ui, rect: Rect, state: &mut ReaderState) {
    if !matches!(state.phase, Phase::Loaded) {
        return;
    }
    let center = rect.right_bottom() - vec2(30.0, 30.0);
    let hit = Rect::from_center_size(center, vec2(44.0, 44.0));
    let layer = egui::LayerId::new(egui::Order::Foreground, egui::Id::new("reader-toggle"));
    let response = ui
        .new_child(egui::UiBuilder::new().max_rect(hit).layer_id(layer))
        .interact(hit, egui::Id::new("reader-toggle-button"), Sense::click());
    let hover = ui.ctx().animate_bool_with_time(response.id, response.hovered(), 0.15);
    // fades out with the cursor, still clickable
    let awake = ui.ctx().animate_bool_with_time(egui::Id::new("reader-awake"), !state.idle(), 0.4);
    let strength = if state.show_chrome { 0.9 } else { (0.32 * awake).max(0.0) + 0.68 * hover };
    if strength <= 0.01 {
        if response.clicked() {
            state.show_chrome = !state.show_chrome;
        }
        return;
    }
    let painter = ui.ctx().layer_painter(layer);
    painter.circle_filled(center, 17.0, Color32::from_rgba_unmultiplied(28, 26, 38, (210.0 * strength) as u8));
    painter.circle_stroke(center, 16.5, Stroke::new(1.0, Color32::from_white_alpha((40.0 * strength) as u8)));
    let glyph = if state.show_chrome { icon::EYE_SLASH } else { icon::SLIDERS_HORIZONTAL };
    painter.text(center, Align2::CENTER_CENTER, glyph, theme::icon_font(16.0), Color32::from_white_alpha((255.0 * strength) as u8));
    let response = response.on_hover_text(if state.show_chrome { t("Hide controls (Tab)") } else { t("Show controls (Tab)") });
    if response.clicked() {
        state.show_chrome = !state.show_chrome;
    }
}

// round glass button of the reader controls, returns true when clicked
fn hud_button(layer: &mut Ui, rect: Rect, id: &str, glyph: &str, active: bool, shown: f32, hint: &str) -> bool {
    let palette = Palette::of(layer.ctx());
    let response = layer.interact(rect, egui::Id::new(id), Sense::click());
    let hover = layer.ctx().animate_bool_with_time(response.id, response.hovered(), 0.12);
    let painter = layer.painter();
    let base = if active { palette.accent } else { Color32::from_rgba_unmultiplied(28, 26, 38, 215) };
    let fill = widgets::lerp_color(base, if active { palette.accent } else { Color32::from_rgba_unmultiplied(48, 45, 64, 230) }, hover);
    let radius = rect.width() / 2.0 + hover;
    painter.circle_filled(rect.center(), radius, fill.gamma_multiply(shown));
    painter.circle_stroke(rect.center(), radius - 0.5, Stroke::new(1.0, Color32::from_white_alpha((30.0 + 30.0 * hover) as u8).gamma_multiply(shown)));
    let fg = if active { theme::readable_on(palette.accent) } else { Color32::WHITE };
    painter.text(rect.center(), Align2::CENTER_CENTER, glyph, theme::icon_font(19.0), fg.gamma_multiply(shown));
    response.on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text(hint).clicked()
}

// returns how far the controls are shown, 0 to 1
fn chrome(ui: &mut Ui, rect: Rect, state: &mut ReaderState, shared: &mut Shared, settings: &crate::store::Settings, keep_open: &mut bool) -> f32 {
    let ctx = ui.ctx().clone();
    // controls stay up while loading or failed so there is always a way out
    let near_edge = !state.idle()
        && ctx.input(|i| i.pointer.hover_pos()).is_some_and(|p| rect.contains(p) && (p.y < rect.top() + TOP_EDGE || p.y > rect.bottom() - BOTTOM_EDGE));
    let visible = state.show_chrome || near_edge || state.chapters_open || !matches!(state.phase, Phase::Loaded);
    let shown = ctx.animate_bool_with_time(egui::Id::new("reader-chrome"), visible, 0.22);
    toggle_button(ui, rect, state);
    if shown <= 0.0 {
        return 0.0;
    }
    let ease = theme::ease_out_cubic(shown);
    let glass = Color32::from_rgba_unmultiplied(28, 26, 38, (215.0 * shown) as u8);
    let stroke = Stroke::new(1.0, Color32::from_white_alpha((30.0 * shown) as u8));
    let text = Color32::from_white_alpha((255.0 * shown) as u8);
    let secondary = Color32::from_white_alpha((160.0 * shown) as u8);

    let mut layer = ui.new_child(egui::UiBuilder::new().max_rect(rect).layer_id(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("reader-ui"))));
    layer.set_opacity(shown);

    // the bars slide in from their edges
    let drop = -16.0 * (1.0 - ease);
    let top = Rect::from_min_size(rect.min + vec2(18.0, 14.0 + drop), vec2(rect.width() - 36.0, 48.0));
    let paged = settings.reader_mode != ReaderMode::Vertical;
    let fullscreen = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
    let size = vec2(46.0, 46.0);
    let close = Rect::from_min_size(top.min, size);
    let mut right = top.right();
    let mut slot = || {
        let r = Rect::from_min_size(pos2(right - size.x, top.top()), size);
        right -= size.x + 10.0;
        r
    };
    let gear = slot();
    let full = slot();
    let list = slot();
    let pages_button = if paged { Some(slot()) } else { None };
    let title_right = pages_button.unwrap_or(list).left() - 10.0;
    let title_width = (title_right - close.right() - 10.0).clamp(0.0, 520.0);
    let title = Rect::from_min_size(pos2(close.right() + 10.0, top.top()), vec2(title_width, size.y));

    if hud_button(&mut layer, close, "reader-close", icon::X, false, shown, t("Close (Esc)")) {
        *keep_open = false;
    }
    if title_width > 60.0 {
        let painter = layer.painter().clone();
        painter.rect_filled(title, CornerRadius::same(255), glass);
        painter.rect_stroke(title, CornerRadius::same(255), stroke, StrokeKind::Inside);
        let clip = painter.with_clip_rect(title.shrink2(vec2(20.0, 0.0)));
        let line_width = title.width() - 44.0;
        widgets::paint_line(&clip, pos2(title.left() + 22.0, title.top() + 7.0), &state.manga_title, theme::bold_font(14.5), text, line_width);
        widgets::paint_line(&clip, pos2(title.left() + 22.0, title.top() + 26.0), &state.chapter.name, theme::font(12.0), secondary, line_width);
    }
    if hud_button(&mut layer, gear, "reader-gear", icon::GEAR_SIX, false, shown, t("Reader settings")) {
        state.settings_open = true;
    }
    let (full_glyph, full_hint) = if fullscreen { (icon::ARROWS_IN, t("Exit full screen (F)")) } else { (icon::ARROWS_OUT, t("Full screen (F)")) };
    if hud_button(&mut layer, full, "reader-fullscreen", full_glyph, false, shown, full_hint) {
        toggle_fullscreen(&ctx);
    }
    if matches!(state.phase, Phase::Loaded) && hud_button(&mut layer, list, "reader-chapters", icon::LIST, state.chapters_open, shown, t("Chapters (C)")) {
        state.chapters_open = !state.chapters_open;
    }
    if let Some(button) = pages_button {
        let glyph = if state.double { icon::BOOK_OPEN } else { icon::FILE };
        if hud_button(&mut layer, button, "reader-two-pages", glyph, state.double, shown, t("Two pages (T)")) {
            toggle_two_pages(state, shared);
        }
    }

    if !matches!(state.phase, Phase::Loaded) {
        return shown;
    }
    let width = (rect.width() - 36.0).min(720.0);
    let bottom = Rect::from_center_size(pos2(rect.center().x, rect.bottom() - 50.0 - drop), vec2(width, 62.0));
    let painter = layer.painter().clone();
    painter.rect_filled(bottom, CornerRadius::same(30), glass);
    painter.rect_stroke(bottom, CornerRadius::same(30), stroke, StrokeKind::Inside);

    let previous = state.previous_chapter();
    let next = state.next_chapter();
    let prev_rect = Rect::from_center_size(bottom.left_center() + vec2(34.0, 0.0), vec2(42.0, 42.0));
    let next_rect = Rect::from_center_size(bottom.right_center() - vec2(34.0, 0.0), vec2(42.0, 42.0));
    for (rect, glyph, enabled, id, hint) in [
        (prev_rect, icon::SKIP_BACK, previous.is_some(), "reader-prev", t("Previous chapter")),
        (next_rect, icon::SKIP_FORWARD, next.is_some(), "reader-next", t("Next chapter")),
    ] {
        let response = layer.interact(rect, egui::Id::new(id), if enabled { Sense::click() } else { Sense::hover() });
        let hover = ctx.animate_bool_with_time(response.id, response.hovered() && enabled, 0.12);
        if hover > 0.0 {
            painter.circle_filled(rect.center(), 20.0, Color32::from_white_alpha((22.0 * hover * shown) as u8));
        }
        painter.text(rect.center(), Align2::CENTER_CENTER, glyph, theme::icon_font(19.0), if enabled { text } else { secondary.gamma_multiply(0.45) });
        if enabled && response.on_hover_text(hint).clicked() {
            let target = if id == "reader-prev" { previous.clone() } else { next.clone() };
            if let Some(target) = target {
                state.go_to(target, &ctx, shared);
                return shown;
            }
        }
    }

    let count = state.pages.len();
    let current = state.position.min(count.saturating_sub(1));
    let slider_rect = Rect::from_min_max(pos2(prev_rect.right() + 14.0, bottom.top() + 11.0), pos2(next_rect.left() - 14.0, bottom.top() + 33.0));
    let label = format!("{} / {}", current + 1, count);
    painter.text(pos2(bottom.center().x, bottom.bottom() - 14.0), Align2::CENTER_CENTER, label, theme::font(12.0), secondary);
    if count > 1 {
        let mut value = current as f32;
        let mut slider_ui = layer.new_child(egui::UiBuilder::new().max_rect(slider_rect));
        slider_ui.spacing_mut().slider_width = slider_rect.width();
        // the reader controls are always dark
        slider_ui.visuals_mut().dark_mode = true;
        let response = widgets::slider(&mut slider_ui, &mut value, 0.0..=(count - 1) as f32, Some(1.0), false);
        if response.changed() {
            let page = value.round() as usize;
            state.position = page;
            if settings.reader_mode == ReaderMode::Vertical {
                state.jump_to = Some(page);
            }
        }
    }
    shown
}

// thin progress line along the bottom and a short page counter after each turn
fn page_feedback(ui: &mut Ui, rect: Rect, state: &ReaderState, settings: &crate::store::Settings, shown: f32) {
    if !matches!(state.phase, Phase::Loaded) || state.pages.is_empty() {
        return;
    }
    let ctx = ui.ctx().clone();
    let palette = Palette::of(&ctx);
    let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Middle, egui::Id::new("reader-feedback")));
    let hidden = 1.0 - shown;
    let count = state.pages.len();
    let fraction = ((state.position + 1).min(count) as f32 / count as f32).clamp(0.0, 1.0);
    let width = theme::spring(&ctx, egui::Id::new(("reader-progress", &state.chapter.id)), rect.width() * fraction, 14.0);
    if settings.reader_progress_bar && hidden > 0.0 {
        let line = Rect::from_min_size(pos2(rect.left(), rect.bottom() - 3.0), vec2(width, 3.0));
        painter.rect_filled(line, CornerRadius::same(2), palette.accent.gamma_multiply(0.85 * hidden));
    }

    let Some(at) = state.turned_at else { return };
    let age = at.elapsed().as_secs_f32();
    if age > 1.6 || state.position >= count {
        return;
    }
    ctx.request_repaint();
    let alpha = (age / 0.15).min(1.0) * ((1.6 - age) / 0.4).clamp(0.0, 1.0) * hidden;
    if alpha <= 0.0 {
        return;
    }
    let label = format!("{} / {}", state.position + 1, count);
    let galley = painter.layout_no_wrap(label, theme::bold_font(13.0), Color32::WHITE);
    let pill = Rect::from_center_size(pos2(rect.center().x, rect.bottom() - 34.0), galley.size() + vec2(26.0, 12.0));
    painter.rect_filled(pill, CornerRadius::same(255), Color32::from_rgba_unmultiplied(20, 19, 28, (190.0 * alpha) as u8));
    painter.galley(pill.center() - galley.size() / 2.0, galley, Color32::WHITE.gamma_multiply(alpha));
}

fn drawer_width(rect: Rect) -> f32 {
    rect.width().min(360.0)
}

// slides in from the right with every chapter, the current one highlighted
fn chapters_drawer(ui: &mut Ui, rect: Rect, state: &mut ReaderState, shared: &mut Shared, open: f32) {
    let ctx = ui.ctx().clone();
    // scroll to the current chapter each time the list opens
    let was_open_id = egui::Id::new("reader-chapters-was-open");
    let was_open = ctx.data(|d| d.get_temp::<bool>(was_open_id)).unwrap_or(false);
    ctx.data_mut(|d| d.insert_temp(was_open_id, state.chapters_open));
    let just_opened = state.chapters_open && !was_open;
    if open <= 0.0 {
        return;
    }
    let ease = theme::ease_out_cubic(open);
    let palette = Palette::new(true, Palette::of(&ctx).accent);
    let width = drawer_width(rect);
    let panel = Rect::from_min_size(pos2(rect.right() - width * ease, rect.top()), vec2(width, rect.height()));
    let mut chosen = None;
    egui::Area::new(egui::Id::new("reader-chapters-area"))
        .order(egui::Order::Foreground)
        .fixed_pos(panel.min)
        .show(&ctx, |ui| {
            ui.set_opacity(ease);
            let painter = ui.painter();
            theme::soft_shadow(painter, panel, 0, 0.0, 30.0, Color32::from_black_alpha(120));
            painter.rect_filled(panel, 0.0, Color32::from_rgb(22, 21, 31));
            painter.vline(panel.left(), panel.y_range(), Stroke::new(1.0, Color32::from_white_alpha(20)));
            let close = Rect::from_min_size(pos2(panel.right() - 16.0 - 38.0, panel.top() + 18.0), vec2(38.0, 38.0));
            if hud_button(ui, close, "reader-chapters-close", icon::X, false, 1.0, t("Close")) {
                state.chapters_open = false;
            }
            let mut inner = ui.new_child(egui::UiBuilder::new().max_rect(panel.shrink2(vec2(16.0, 18.0))));
            inner.style_mut().visuals.override_text_color = Some(Color32::WHITE);
            inner.label(RichText::new(t("Chapters")).font(theme::bold_font(19.0)));
            inner.label(RichText::new(tf("{n} chapters", &[("n", &state.chapters.len())])).size(12.0).color(palette.secondary));
            inner.add_space(10.0);
            if state.chapters.is_empty() {
                inner.add_space(30.0);
                inner.vertical_centered(|ui| ui.add(egui::Spinner::new().size(24.0).color(Color32::WHITE)));
                return;
            }
            egui::ScrollArea::vertical().id_salt("reader-chapters-scroll").auto_shrink([false, false]).show(&mut inner, |ui| {
                ui.spacing_mut().item_spacing.y = 4.0;
                for chapter in &state.chapters {
                    let current = chapter.id == state.chapter.id;
                    let read = shared.store.progress.get(&chapter.id).is_some_and(|p| p.completed);
                    let (row, response) = ui.allocate_exact_size(vec2(ui.available_width(), 46.0), Sense::click());
                    let hover = ui.ctx().animate_bool_with_time(response.id, response.hovered(), 0.1);
                    if current {
                        ui.painter().rect_filled(row, CornerRadius::same(12), palette.accent.gamma_multiply(0.25));
                    } else if hover > 0.0 {
                        ui.painter().rect_filled(row, CornerRadius::same(12), Color32::from_white_alpha((14.0 * hover) as u8));
                    }
                    let color = if current { Color32::WHITE } else if read { palette.tertiary } else { palette.text };
                    widgets::paint_line(ui.painter(), pos2(row.left() + 14.0, row.top() + 6.0), &chapter.name, theme::bold_font(14.0), color, row.width() - 60.0);
                    if let Some(date) = chapter.date {
                        let date = crate::model::short_date(date);
                        widgets::paint_line(ui.painter(), pos2(row.left() + 14.0, row.top() + 25.0), &date, theme::font(11.5), palette.tertiary, row.width() - 60.0);
                    }
                    if read {
                        ui.painter().text(row.right_center() - vec2(18.0, 0.0), Align2::CENTER_CENTER, icon::CHECK, theme::icon_font(15.0), palette.tertiary);
                    } else if current {
                        ui.painter().circle_filled(row.right_center() - vec2(18.0, 0.0), 4.0, palette.accent);
                    }
                    if current && just_opened {
                        response.scroll_to_me(Some(egui::Align::Center));
                    }
                    if response.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() && !current {
                        chosen = Some(chapter.clone());
                    }
                }
            });
        });
    if let Some(chapter) = chosen {
        state.chapters_open = false;
        state.go_to(chapter, &ctx, shared);
    }
}

fn settings_sheet(ui: &mut Ui, state: &mut ReaderState, shared: &mut Shared) {
    if !state.settings_open {
        return;
    }
    let palette = Palette::of(ui.ctx());
    let modal = egui::Modal::new(egui::Id::new("reader-settings"))
        .frame(theme::modal_frame(&palette))
        .show(ui.ctx(), |ui| {
            ui.set_width(460.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new(t("Reader")).font(theme::bold_font(22.0)));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if widgets::primary_button(ui, t("Done"), None, Some(90.0), true).clicked() {
                        ui.close();
                    }
                });
            });
            ui.add_space(10.0);
            let height = (ui.ctx().content_rect().height() - 170.0).max(200.0);
            egui::ScrollArea::vertical().max_height(height).min_scrolled_height(height).show(ui, |ui| {
                reader_options(ui, shared);
            });
        });
    if modal.should_close() {
        state.settings_open = false;
    }
}

// shown in the reader and in settings
pub fn reader_options(ui: &mut Ui, shared: &mut Shared) {
    let palette = Palette::of(ui.ctx());
    let mut changed = false;
    let settings = &mut shared.store.settings;

    ui.label(RichText::new(t("Reading mode")).font(theme::bold_font(14.5)));
    ui.horizontal_wrapped(|ui| {
        for mode in ReaderMode::ALL {
            let glyph = match mode {
                ReaderMode::Vertical => icon::ARROWS_DOWN_UP,
                ReaderMode::PagedLtr => icon::ARROW_RIGHT,
                ReaderMode::PagedRtl => icon::ARROW_LEFT,
            };
            if widgets::pill(ui, mode.title(), Some(glyph), settings.reader_mode == mode, 0.0).clicked() {
                settings.reader_mode = mode;
                changed = true;
            }
        }
    });
    ui.add_space(10.0);

    ui.label(RichText::new(t("Two pages")).font(theme::bold_font(14.5)));
    ui.horizontal_wrapped(|ui| {
        for option in TwoPages::ALL {
            if widgets::pill(ui, option.title(), None, settings.two_pages == option, 0.0).clicked() {
                settings.two_pages = option;
                changed = true;
            }
        }
    });
    ui.label(RichText::new(t("In paged mode two pages sit side by side, by themselves when the window is wide enough.")).size(12.0).color(palette.tertiary));
    ui.add_space(10.0);

    ui.label(RichText::new(t("Page width")).font(theme::bold_font(14.5)));
    ui.horizontal(|ui| {
        ui.label(RichText::new(icon::FILE).color(palette.secondary));
        ui.spacing_mut().slider_width = (ui.available_width() - 70.0).clamp(120.0, 320.0);
        changed |= widgets::slider(ui, &mut settings.reader_width, 360.0..=2400.0, None, true).changed();
        ui.label(RichText::new(icon::ARROWS_OUT).color(palette.secondary));
    });
    ui.label(RichText::new(t("Vertical mode. Ctrl + mouse wheel changes it while reading.")).size(12.0).color(palette.tertiary));
    ui.add_space(8.0);

    changed |= widgets::toggle_row(ui, &mut settings.tap_zones, t("Tap zones: click the left or right edge to turn the page"));
    changed |= widgets::toggle_row(ui, &mut settings.page_animation, t("Page turn animation"));
    changed |= widgets::toggle_row(ui, &mut settings.reader_progress_bar, t("Progress line at the bottom"));
    ui.add_space(6.0);

    ui.label(RichText::new(t("Background")).font(theme::bold_font(14.5)));
    ui.horizontal(|ui| {
        for background in ReaderBackground::ALL {
            let selected = settings.reader_background == background;
            let (rect, response) = ui.allocate_exact_size(vec2(64.0, 56.0), Sense::click());
            ui.painter().circle_filled(rect.center_top() + vec2(0.0, 18.0), 16.0, background.color());
            ui.painter().circle_stroke(
                rect.center_top() + vec2(0.0, 18.0),
                17.0,
                Stroke::new(if selected { 2.5 } else { 1.0 }, if selected { palette.accent } else { palette.glass_stroke }),
            );
            ui.painter().text(rect.center_bottom() - vec2(0.0, 6.0), Align2::CENTER_CENTER, background.name(), theme::font(11.5), palette.secondary);
            if response.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                settings.reader_background = background;
                changed = true;
            }
        }
    });
    ui.add_space(6.0);

    ui.label(RichText::new(t("Dim pages")).font(theme::bold_font(14.5)));
    ui.horizontal(|ui| {
        ui.label(RichText::new(icon::SUN).color(palette.secondary));
        changed |= widgets::slider(ui, &mut settings.reader_dim, 0.0..=0.7, None, false).changed();
        ui.label(RichText::new(icon::MOON).color(palette.secondary));
    });
    ui.add_space(6.0);
    changed |= widgets::toggle_row(ui, &mut settings.reader_gap, t("Gap between pages (vertical mode)"));
    changed |= widgets::toggle_row(ui, &mut settings.keep_awake, t("Keep screen on while reading"));
    ui.add_space(6.0);
    ui.label(
        RichText::new(t("Keys: ← → or A D turn pages · Space / PgDn scroll · hold ↑ ↓ to scroll smoothly · T two pages · C chapters · F full screen · Tab shows the controls · Esc close"))
            .size(12.0)
            .color(palette.tertiary),
    );

    if changed {
        shared.store.settings_changed();
    }
}

#[cfg(test)]
mod tests {
    use super::{Spread, settle, spread_from, spread_to, step};

    // every spread from the first page to the end of the chapter
    fn walk_forward(count: usize, wide: impl Fn(usize) -> bool) -> Vec<(usize, Option<usize>)> {
        let mut out = Vec::new();
        let mut current = Some(spread_from(0, count, &wide));
        while let Some(spread) = current {
            out.push((spread.first, spread.second));
            current = step(Some(spread), 1, count, &wide);
        }
        out
    }

    #[test]
    fn the_cover_and_wide_pages_stand_alone() {
        assert_eq!(walk_forward(5, |_| false), vec![(0, None), (1, Some(2)), (3, Some(4))]);
        // page 3 is a two-page drawing, so 2 can't pair with it
        assert_eq!(walk_forward(6, |i| i == 3), vec![(0, None), (1, Some(2)), (3, None), (4, Some(5))]);
        assert_eq!(walk_forward(4, |i| i == 2), vec![(0, None), (1, None), (2, None), (3, None)]);
        // an odd page at the end stands alone
        assert_eq!(walk_forward(4, |_| false), vec![(0, None), (1, Some(2)), (3, None)]);
    }

    #[test]
    fn turning_back_retraces_the_spreads() {
        let wide = |i: usize| i == 3;
        let mut back = Vec::new();
        let mut current = step(None, -1, 7, &wide);
        while let Some(spread) = current {
            back.push((spread.first, spread.second));
            if spread.first == 0 {
                break;
            }
            current = step(Some(spread), -1, 7, &wide);
        }
        assert_eq!(back, vec![(5, Some(6)), (4, None), (3, None), (1, Some(2)), (0, None)]);
    }

    // the bug: a page's shape became known while reading, the pairing shifted and a page was
    // shown again in the next spread
    #[test]
    fn a_page_never_shows_twice_in_a_row() {
        let count = 12;
        for revealed in 0..count {
            for reveal_at in 0..6 {
                let mut seen: Vec<usize> = Vec::new();
                let mut current = Some(spread_from(0, count, &|_| false));
                let mut turns = 0;
                while let Some(spread) = current {
                    let wide = |i: usize| turns >= reveal_at && i == revealed;
                    let spread = settle(spread, count, &wide);
                    for page in [Some(spread.first), spread.second].into_iter().flatten() {
                        assert!(!seen.contains(&page), "page {page} again (wide page {revealed}, known after {reveal_at} turns)");
                        seen.push(page);
                    }
                    current = step(Some(spread), 1, count, &wide);
                    turns += 1;
                }
                // and nothing was skipped
                seen.sort();
                assert_eq!(seen, (0..count).collect::<Vec<_>>());
            }
        }
    }

    #[test]
    fn a_pair_made_by_turning_back_keeps_the_later_page() {
        // the pair 3 and 4 was made before page 3 turned out to be wide
        let made = spread_to(4, 8, &|_| false);
        assert_eq!(made, Spread { first: 3, second: Some(4), from_end: true });
        let wide = |i: usize| i == 3;
        let settled = settle(made, 8, &wide);
        assert_eq!((settled.first, settled.second), (4, None));
        let back = step(Some(settled), -1, 8, &wide).unwrap();
        assert_eq!((back.first, back.second), (3, None));
    }
}
