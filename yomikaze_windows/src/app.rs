// window, navigation rail, background, overlays and routing

use std::collections::HashMap;
use std::time::Instant;

use egui::{Align2, Color32, Context, CornerRadius, Id, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, Ui, Vec2, pos2, vec2};
use crate::icons as icon;
use poll_promise::Promise;

use crate::downloads::DownloadManager;
use crate::images::{Images, Kind, State};
use crate::model::{Chapter, MangaSummary, SavedManga};
use crate::store::{self, Store};
use crate::theme::{self, Palette};
use crate::ui::{browse, detail, history, library, reader, search, settings, stats, widgets};
use crate::anilist::{self, AniList};
use crate::{hello, net};
use crate::i18n::{t, tf};

const SIDEBAR_COMPACT: f32 = 76.0;
const SIDEBAR_WIDE: f32 = 232.0;
// from this window width the sidebar opens up and shows labels
const SIDEBAR_WIDE_FROM: f32 = 1080.0;


// the version the app shows. the suffix marks a fix release of the same version, the update
// check keeps comparing the plain cargo version. drop it at the next version.
pub const VERSION_LABEL: &str = concat!(env!("CARGO_PKG_VERSION"), " hotfix");

// runs work on a background thread and wakes the ui when done
pub fn spawn<T: Send + 'static>(ctx: &Context, work: impl FnOnce() -> T + Send + 'static) -> Promise<T> {
    let (sender, promise) = Promise::new();
    let ctx = ctx.clone();
    std::thread::spawn(move || {
        sender.send(work());
        ctx.request_repaint();
    });
    promise
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Tab {
    Library,
    Browse,
    Search,
    History,
    Settings,
}

impl Tab {
    const ALL: [Tab; 5] = [Tab::Library, Tab::Browse, Tab::Search, Tab::History, Tab::Settings];

    fn title(self) -> &'static str {
        match self {
            Tab::Library => t("Library"),
            Tab::Browse => t("Browse"),
            Tab::Search => t("Search"),
            Tab::History => t("History"),
            Tab::Settings => t("Settings"),
        }
    }

    fn glyph(self) -> &'static str {
        match self {
            Tab::Library => icon::BOOKS,
            Tab::Browse => icon::COMPASS,
            Tab::Search => icon::MAGNIFYING_GLASS,
            Tab::History => icon::CLOCK_COUNTER_CLOCKWISE,
            Tab::Settings => icon::GEAR_SIX,
        }
    }
}

#[derive(Clone, Debug)]
pub enum Route {
    Detail(MangaSummary),
    Stats,
    Credits,
    Changelog,
}

#[derive(Clone)]
pub struct ReaderRequest {
    pub manga_id: String,
    pub manga_title: String,
    // newest first, may be empty, the reader loads it then
    pub chapters: Vec<Chapter>,
    pub chapter: Chapter,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum LibrarySection {
    Saved,
    Downloads,
}

pub struct Router {
    pub tab: Tab,
    stacks: HashMap<Tab, Vec<Route>>,
    pub reader: Option<ReaderRequest>,
    pub library_section: LibrarySection,
    // used for the page fade
    pub pushed_at: Instant,
    pub theme_dirty: bool,
    pub toast: Option<(String, Instant, &'static str)>,
    // set by the YOMIKAZE_START test switch
    pub search_text: Option<String>,
}

impl Router {
    fn new() -> Self {
        Self {
            tab: Tab::Library,
            stacks: HashMap::new(),
            reader: None,
            library_section: LibrarySection::Saved,
            pushed_at: Instant::now(),
            theme_dirty: true,
            toast: None,
            search_text: None,
        }
    }

    pub fn push(&mut self, route: Route) {
        self.stacks.entry(self.tab).or_default().push(route);
        self.pushed_at = Instant::now();
    }

    pub fn open(&mut self, manga: &MangaSummary) {
        self.push(Route::Detail(manga.clone()));
    }

    pub fn pop(&mut self) {
        if let Some(stack) = self.stacks.get_mut(&self.tab) {
            stack.pop();
        }
        self.pushed_at = Instant::now();
    }

    pub fn top(&self) -> Option<&Route> {
        self.stacks.get(&self.tab).and_then(|s| s.last())
    }

    pub fn select(&mut self, tab: Tab) {
        if self.tab == tab {
            // clicking the current tab again goes back to its root
            if self.stacks.remove(&tab).is_none() {
                return;
            }
        }
        self.tab = tab;
        self.pushed_at = Instant::now();
    }

    pub fn open_reader(&mut self, chapter: &Chapter, chapters: &[Chapter], manga_id: &str, manga_title: &str) {
        self.reader = Some(ReaderRequest {
            manga_id: manga_id.to_string(),
            manga_title: manga_title.to_string(),
            chapters: chapters.to_vec(),
            chapter: chapter.clone(),
        });
    }

    pub fn resume(&mut self, manga: &SavedManga) {
        if manga.last_chapter_id.is_empty() {
            return;
        }
        let cached = store::catalog::chapters(&manga.manga_id);
        let chapter = cached
            .iter()
            .find(|c| c.id == manga.last_chapter_id)
            .cloned()
            .unwrap_or_else(|| Chapter { id: manga.last_chapter_id.clone(), name: manga.last_chapter_name.clone(), date: None });
        self.reader = Some(ReaderRequest {
            manga_id: manga.manga_id.clone(),
            manga_title: manga.title.clone(),
            chapters: cached,
            chapter,
        });
    }

    pub fn show_downloads(&mut self) {
        self.library_section = LibrarySection::Downloads;
        self.stacks.remove(&Tab::Library);
        self.tab = Tab::Library;
    }

    pub fn toast(&mut self, text: impl Into<String>) {
        self.toast = Some((text.into(), Instant::now(), icon::CHECK_CIRCLE));
    }

    pub fn toast_warning(&mut self, text: impl Into<String>) {
        self.toast = Some((text.into(), Instant::now(), icon::WARNING_CIRCLE));
    }

    pub fn toast_offline(&mut self) {
        self.toast = Some((t("You're offline").to_string(), Instant::now(), icon::WIFI_SLASH));
    }
}

pub struct Shared {
    pub store: Store,
    pub images: Images,
    pub downloads: DownloadManager,
    pub router: Router,
    pub hwnd: isize,
    pub anilist: AniList,
    pub gallery: crate::wallpapers::Gallery,
    // the anilist login, kept in memory while connected
    pub anilist_token: Option<String>,
    // the browser login is open and the app waits for the yomikaze:// link
    pub anilist_waiting: bool,
    // run an anilist sync soon, true for a full one that also retries unmatched series
    pub anilist_sync_request: Option<bool>,
}

// screens keep their state here so switching tabs keeps scroll positions and results
#[derive(Default)]
struct Screens {
    library: library::LibraryState,
    browse: browse::BrowseState,
    search: search::SearchState,
    history: history::HistoryState,
    settings: settings::SettingsState,
    details: HashMap<String, detail::DetailState>,
}

pub struct YomikazeApp {
    shared: Shared,
    screens: Screens,
    reader: Option<reader::ReaderState>,
    started: Instant,
    splash_done: bool,
    // last colour sent to windows, so dwm is only called when it changes
    title_bar: Option<[u8; 3]>,
    anilist_next_sync: Option<Instant>,
    // series changed in the app, pushed to anilist at anilist_push_at
    anilist_pending: std::collections::HashSet<String>,
    anilist_push_at: Option<Instant>,
    // finished series waiting for the rating prompt: (title, anilist id)
    anilist_finished: Vec<(String, i64)>,
    // the fonts are ordered for chinese
    fonts_chinese: bool,
    anilist_poll: Instant,
    show_changelog: bool,
    locked: bool,
    unlocking: Option<Promise<bool>>,
    was_minimized: bool,
    was_online: bool,
    sidebar_width: f32,
}

impl YomikazeApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let ctx = &cc.egui_ctx;
        theme::install_fonts(ctx, false);
        egui_extras_install(ctx);
        net::start_monitor(ctx.clone());

        let store = Store::load();
        let hwnd = window_handle(cc);
        let locked = store.settings.app_lock;
        let mut show_changelog = store.settings.last_seen_version != VERSION_LABEL;

        // developer switch to open a screen directly, e.g. YOMIKAZE_START=browse or detail:<id>
        let mut router = Router::new();
        if let Ok(start) = std::env::var("YOMIKAZE_START") {
            show_changelog = false;
            let (what, arg) = start.split_once(':').unwrap_or((start.as_str(), ""));
            match what {
                "browse" => router.tab = Tab::Browse,
                "search" => {
                    router.tab = Tab::Search;
                    if !arg.is_empty() {
                        router.search_text = Some(arg.to_string());
                    }
                }
                "history" => router.tab = Tab::History,
                "settings" => router.tab = Tab::Settings,
                "downloads" => router.show_downloads(),
                "credits" => {
                    router.tab = Tab::Settings;
                    router.push(Route::Credits);
                }
                "stats" => {
                    router.tab = Tab::History;
                    router.push(Route::Stats);
                }
                "detail" => {
                    router.tab = Tab::Browse;
                    router.push(Route::Detail(MangaSummary::new(arg, "")));
                }
                "changelog" => show_changelog = true,
                "read" => {
                    if let Some((manga, chapter)) = arg.split_once(':') {
                        let chapter = Chapter { id: chapter.to_string(), name: "Chapter".to_string(), date: None };
                        router.open_reader(&chapter, &[], manga, "Test");
                    }
                }
                _ => {}
            }
        }

        let start_settings = std::env::var("YOMIKAZE_START")
            .ok()
            .and_then(|v| v.strip_prefix("settings:").map(settings::SettingsState::starting_at))
            .unwrap_or_default();
        let gallery = crate::wallpapers::Gallery::new(ctx);
        let anilist_token = store.settings.anilist_user.as_ref().and_then(|_| anilist::load_token());
        Self {
            shared: Shared {
                images: Images::new(ctx),
                downloads: DownloadManager::new(ctx),
                router,
                store,
                hwnd,
                anilist: AniList::start(ctx),
                gallery,
                anilist_token,
                anilist_waiting: false,
                anilist_sync_request: None,
            },
            screens: Screens { settings: start_settings, ..Default::default() },
            reader: None,
            started: Instant::now(),
            splash_done: false,
            show_changelog,
            locked,
            unlocking: None,
            was_minimized: false,
            was_online: true,
            sidebar_width: 0.0,
            title_bar: None,
            anilist_next_sync: Some(Instant::now() + std::time::Duration::from_secs(4)),
            anilist_poll: Instant::now(),
            anilist_pending: Default::default(),
            anilist_push_at: None,
            anilist_finished: Vec::new(),
            fonts_chinese: false,
        }
    }

    fn handle_background_events(&mut self, ctx: &Context) {
        for done in self.shared.downloads.take_finished() {
            self.shared.store.downloads.insert(done.chapter_id.clone(), done);
            self.shared.store.mark_dirty();
        }

        // retry failed images when the connection returns, toast when it drops
        let online = net::is_online();
        if online != self.was_online {
            if online {
                self.shared.images.retry_failed();
            } else if self.splash_done {
                self.shared.router.toast_offline();
            }
            self.was_online = online;
        }

        // app lock: lock when minimized, ask again on restore
        let minimized = ctx.input(|i| i.viewport().minimized.unwrap_or(false));
        if minimized && !self.was_minimized && self.shared.store.settings.app_lock {
            self.locked = true;
        }
        if !minimized && self.was_minimized && self.locked {
            self.start_unlock(ctx);
        }
        self.was_minimized = minimized;

        self.anilist_frame();

        let awake = self.reader.is_some() && self.shared.store.settings.keep_awake;
        hello::keep_display_on(awake);

        self.shared.store.flush(false);
    }

    fn anilist_frame(&mut self) {
        use std::time::Duration;
        let shared = &mut self.shared;

        // the login link leaves the token in a file, look for it while waiting
        if shared.anilist_waiting && self.anilist_poll.elapsed() > Duration::from_millis(800) {
            self.anilist_poll = Instant::now();
            if let Some(token) = anilist::take_pending_login() {
                shared.anilist_waiting = false;
                shared.anilist.login(token);
            }
        }

        for event in shared.anilist.take_events() {
            match event {
                anilist::Event::LoggedIn { token, user } => {
                    anilist::save_token(&token);
                    shared.router.toast(tf("Connected to AniList as {name}", &[("name", &user.name)]));
                    shared.anilist_token = Some(token);
                    shared.store.settings.anilist_user = Some(user);
                    shared.store.settings.anilist_skip.clear();
                    shared.store.settings_changed();
                    self.anilist_next_sync = Some(Instant::now());
                }
                anilist::Event::LoginFailed(error) => shared.router.toast_warning(error),
                anilist::Event::Synced(outcome) => {
                    let imported = outcome.imported.len();
                    let finished = shared.store.apply_anilist(outcome);
                    self.anilist_finished.extend(finished);
                    if imported > 0 {
                        shared.router.toast(tf("Added {n} series from AniList", &[("n", &imported)]));
                    }
                }
                anilist::Event::TokenRejected => {
                    anilist::delete_token();
                    shared.anilist.clear();
                    shared.anilist_token = None;
                    shared.store.settings.anilist_user = None;
                    shared.store.settings_changed();
                    shared.router.toast_warning(t("The AniList login has expired. Connect again in Settings."));
                }
            }
        }

        // changes go up a moment later, just for the series that changed
        if !shared.store.sync_changed.is_empty() {
            self.anilist_pending.extend(shared.store.sync_changed.drain());
            self.anilist_push_at = Some(Instant::now() + Duration::from_millis(1500));
        }
        let push_due = self.anilist_push_at.is_some_and(|at| Instant::now() >= at);
        if push_due && net::is_online() {
            if let (Some(token), Some(user)) = (&shared.anilist_token, &shared.store.settings.anilist_user) {
                let local = shared.store.anilist_locals(Some(&self.anilist_pending));
                shared.anilist.push(token, user, local);
            }
            self.anilist_pending.clear();
            self.anilist_push_at = None;
        }
        let full = shared.anilist_sync_request.take();
        let due = self.anilist_next_sync.is_some_and(|at| Instant::now() >= at) || full.is_some();
        if due && self.splash_done && net::is_online() {
            if let (Some(token), Some(user)) = (&shared.anilist_token, &shared.store.settings.anilist_user) {
                let skip = if full == Some(true) { Default::default() } else { shared.store.settings.anilist_skip.iter().copied().collect() };
                shared.anilist.sync(token, user, shared.store.anilist_locals(None), skip);
            }
            self.anilist_next_sync = Some(Instant::now() + Duration::from_secs(15 * 60));
        }
    }

    // the title bar matches the top of the app, or the reader background while reading
    fn style_title_bar(&mut self, ctx: &Context) {
        let palette = Palette::of(ctx);
        let color = if self.shared.router.reader.is_some() {
            self.shared.store.settings.reader_background.color()
        } else {
            theme::top_color(&palette)
        };
        let rgb = [color.r(), color.g(), color.b()];
        if self.title_bar == Some(rgb) {
            return;
        }
        self.title_bar = Some(rgb);
        let text = theme::readable_on(color);
        let dark = text == Color32::WHITE;
        hello::style_title_bar(self.shared.hwnd, dark, rgb, [text.r(), text.g(), text.b()]);
    }

    fn start_unlock(&mut self, ctx: &Context) {
        if self.unlocking.is_some() {
            return;
        }
        let hwnd = self.shared.hwnd;
        self.unlocking = Some(spawn(ctx, move || hello::verify(hwnd, t("Unlock Yomikaze"))));
    }
}

fn window_handle(cc: &eframe::CreationContext<'_>) -> isize {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    cc.window_handle()
        .ok()
        .and_then(|h| match h.as_raw() {
            RawWindowHandle::Win32(win) => Some(win.hwnd.get()),
            _ => None,
        })
        .unwrap_or(0)
}

fn egui_extras_install(ctx: &Context) {
    ctx.options_mut(|o| o.zoom_with_keyboard = true);
}

impl eframe::App for YomikazeApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if self.shared.router.theme_dirty {
            theme::apply(&ctx, &self.shared.store.settings);
            crate::i18n::set_language(&self.shared.store.settings.language);
            let chinese = crate::i18n::is_chinese();
            if chinese != self.fonts_chinese {
                theme::install_fonts(&ctx, chinese);
                self.fonts_chinese = chinese;
            }
            ctx.set_zoom_factor(self.shared.store.settings.ui_scale.clamp(0.75, 1.6));
            self.shared.router.theme_dirty = false;
        }
        self.style_title_bar(&ctx);
        self.shared.images.begin_frame(&ctx);
        self.handle_background_events(&ctx);

        let palette = Palette::of(&ctx);
        let full = ui.max_rect();
        ui.painter().rect_filled(full, 0.0, palette.background);

        if let Some(request) = self.shared.router.reader.clone() {
            let state = match &mut self.reader {
                Some(state) if state.owns(&request) => state,
                _ => {
                    self.reader = Some(reader::ReaderState::new(request, &ctx, &self.shared));
                    self.reader.as_mut().expect("just set")
                }
            };
            if !reader::show(ui, state, &mut self.shared) {
                state.close(&mut self.shared);
                self.reader = None;
                self.shared.router.reader = None;
            }
        } else {
            self.reader = None;
            self.main_layout(ui, &palette);
        }

        self.overlays(&ctx, &palette);
    }

    fn on_exit(&mut self) {
        if let Some(state) = &mut self.reader {
            state.close(&mut self.shared);
        }
        hello::keep_display_on(false);
        self.shared.store.flush(true);
    }
}

impl YomikazeApp {
    fn main_layout(&mut self, ui: &mut Ui, palette: &Palette) {
        let ctx = ui.ctx().clone();
        let full = ui.max_rect();
        let target = if full.width() >= SIDEBAR_WIDE_FROM { SIDEBAR_WIDE } else { SIDEBAR_COMPACT };
        let width = theme::spring(&ctx, Id::new("sidebar-width"), target, 16.0);
        self.sidebar_width = width;
        let sidebar = Rect::from_min_size(full.min, vec2(width, full.height()));
        let content = Rect::from_min_max(pos2(sidebar.right(), full.min.y), full.max);

        self.paint_background(ui, full, palette);

        // a new page rises a little while it fades in
        let t = (self.shared.router.pushed_at.elapsed().as_secs_f32() / 0.24).min(1.0);
        if t < 1.0 {
            ctx.request_repaint();
        }
        let ease = theme::ease_out_cubic(t);
        let moved = content.translate(vec2(0.0, 12.0 * (1.0 - ease)));
        let mut content_ui = ui.new_child(egui::UiBuilder::new().max_rect(moved).id_salt("content"));
        content_ui.set_clip_rect(content);
        content_ui.set_opacity(0.25 + 0.75 * ease);
        self.route(&mut content_ui);

        let open = ((width - SIDEBAR_COMPACT) / (SIDEBAR_WIDE - SIDEBAR_COMPACT)).clamp(0.0, 1.0);
        self.sidebar(ui, sidebar, palette, open);
    }

    fn route(&mut self, ui: &mut Ui) {
        let shared = &mut self.shared;
        let screens = &mut self.screens;
        let rect = ui.max_rect();
        let mut inner = ui.new_child(egui::UiBuilder::new().max_rect(rect).id_salt(("route", shared.router.tab as u8)));

        match shared.router.top().cloned() {
            Some(Route::Detail(manga)) => {
                let state = screens
                    .details
                    .entry(manga.id.clone())
                    .or_insert_with(|| detail::DetailState::new(manga.clone()));
                detail::show(&mut inner, state, shared);
            }
            Some(Route::Stats) => stats::show(&mut inner, shared),
            Some(Route::Credits) => settings::credits(&mut inner, shared),
            Some(Route::Changelog) => settings::changelog_page(&mut inner, shared),
            None => match shared.router.tab {
                Tab::Library => library::show(&mut inner, &mut screens.library, shared),
                Tab::Browse => browse::show(&mut inner, &mut screens.browse, shared),
                Tab::Search => search::show(&mut inner, &mut screens.search, shared),
                Tab::History => history::show(&mut inner, &mut screens.history, shared),
                Tab::Settings => settings::show(&mut inner, &mut screens.settings, shared),
            },
        }
    }

    fn paint_background(&mut self, ui: &mut Ui, rect: Rect, palette: &Palette) {
        let settings = &self.shared.store.settings;
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, palette.background);

        if settings.wallpaper_rev > 0 {
            let url = store::wallpaper_url(settings.wallpaper_rev);
            let (zoom, ox, oy, dim) = (settings.wallpaper_zoom, settings.wallpaper_x, settings.wallpaper_y, settings.wallpaper_dim);
            if let State::Ready(loaded) = self.shared.images.get(&url, Kind::Wide) {
                paint_wallpaper(ui, &painter, loaded.first().id(), loaded.size, rect, zoom, ox, oy);
                painter.rect_filled(rect, 0.0, palette.background.gamma_multiply(dim));
                return;
            }
        }
        theme::backdrop(&painter, rect, palette);
    }

    // open is 0 for the icon column and 1 for the full sidebar with labels
    fn sidebar(&mut self, ui: &mut Ui, rect: Rect, palette: &Palette, open: f32) {
        let ctx = ui.ctx().clone();
        let painter = ui.painter_at(rect);
        // more solid over a background picture so the menu stays easy to read
        let picture = self.shared.store.settings.wallpaper_rev > 0;
        let surface = match (palette.dark, picture) {
            (true, false) => Color32::from_rgba_unmultiplied(8, 7, 14, 120),
            (true, true) => Color32::from_rgba_unmultiplied(12, 11, 18, 215),
            (false, false) => Color32::from_rgba_unmultiplied(255, 255, 255, 150),
            (false, true) => Color32::from_rgba_unmultiplied(250, 250, 253, 225),
        };
        painter.rect_filled(rect, 0.0, surface);
        painter.vline(rect.right() - 0.5, rect.y_range(), Stroke::new(1.0, palette.glass_stroke));

        // labels fade in once there is room for them
        let labels = ((open - 0.45) / 0.55).clamp(0.0, 1.0);
        let compact = labels < 0.5;
        let column = rect.left() + SIDEBAR_COMPACT / 2.0;
        let label_x = rect.left() + 66.0;
        let label_room = (rect.right() - label_x - 16.0).max(10.0);
        let row_left = rect.left() + 12.0;
        let row_right = rect.right() - 12.0;

        let logo = Rect::from_center_size(pos2(column, rect.top() + 40.0), Vec2::splat(40.0));
        theme::rounded_gradient(&painter, logo, 12.0, theme::mix(palette.accent, Color32::WHITE, 0.28), palette.accent);
        painter.text(logo.center(), Align2::CENTER_CENTER, "読", theme::heavy_font(22.0), theme::readable_on(palette.accent));
        let version = tf("Version {version}", &[("version", &VERSION_LABEL)]);
        if labels > 0.0 {
            wordmark(&painter, pos2(label_x, logo.top() - 2.0), palette, labels);
        }
        let logo_response = ui.interact(logo, Id::new("rail-logo"), Sense::hover());
        let logo_hover = ctx.animate_bool_with_time(logo_response.id, logo_response.hovered() && compact, 0.15);
        rail_flyout(&ctx, logo.center(), rect.right(), logo_hover, "Yomikaze", Some(&version), palette);

        let first = rect.top() + 106.0;
        let step = 48.0;
        let selected_index = Tab::ALL.iter().position(|t| *t == self.shared.router.tab).unwrap_or(0);
        let target = first + step * selected_index as f32;
        let y = theme::spring(&ctx, Id::new("rail-indicator"), target, 20.0);
        let selected_row = Rect::from_min_max(pos2(row_left, y - 21.0), pos2(row_right, y + 21.0));
        painter.rect_filled(selected_row, CornerRadius::same(12), palette.accent.gamma_multiply(if palette.dark { 0.17 } else { 0.13 }));
        // the marker stretches while it travels to the new tab
        let stretch = (target - y).abs().min(26.0);
        painter.rect_filled(Rect::from_center_size(pos2(row_left + 3.0, y), vec2(3.5, 16.0 + stretch)), CornerRadius::same(2), palette.accent);

        for (index, tab) in Tab::ALL.into_iter().enumerate() {
            let center_y = first + step * index as f32;
            let row = Rect::from_min_max(pos2(row_left, center_y - 21.0), pos2(row_right, center_y + 21.0));
            let response = ui.interact(row, Id::new(("rail-tab", index)), Sense::click());
            let hover = ctx.animate_bool_with_time(response.id, response.hovered(), 0.12);
            let selected = index == selected_index;
            if hover > 0.0 && !selected {
                painter.rect_filled(row, CornerRadius::same(12), palette.quaternary.gamma_multiply(hover));
            }
            let color = if selected { palette.accent } else { widgets::lerp_color(palette.secondary, palette.text, hover) };
            painter.text(pos2(column + 1.5 * hover, center_y), Align2::CENTER_CENTER, tab.glyph(), theme::icon_font(21.0), color);
            if labels > 0.0 {
                let text = if selected { palette.text } else { widgets::lerp_color(palette.secondary, palette.text, hover) };
                widgets::paint_line(&painter, pos2(label_x, center_y - 10.0), tab.title(), theme::bold_font(14.5), text.gamma_multiply(labels), label_room - 30.0);
            }

            if tab == Tab::Library {
                let active = self.shared.downloads.active_count();
                if active > 0 {
                    let fg = theme::readable_on(palette.accent);
                    if compact {
                        let badge = pos2(column + 13.0, center_y - 11.0);
                        painter.circle_filled(badge, 8.5, palette.accent);
                        painter.text(badge, Align2::CENTER_CENTER, active.to_string(), theme::bold_font(10.0), fg);
                    } else {
                        let galley = painter.layout_no_wrap(active.to_string(), theme::bold_font(11.0), fg);
                        let pill = Rect::from_center_size(pos2(row.right() - 22.0, center_y), vec2(galley.size().x + 14.0, 20.0));
                        painter.rect_filled(pill, CornerRadius::same(255), palette.accent.gamma_multiply(labels));
                        painter.galley(pill.center() - galley.size() / 2.0, galley, fg);
                    }
                }
            }
            let flyout = if compact { hover } else { 0.0 };
            rail_flyout(&ctx, pos2(column, center_y), rect.right(), flyout, tab.title(), None, palette);
            if response.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                self.shared.router.select(tab);
            }
        }

        // bottom group, laid out from the bottom up
        let mut bottom = rect.bottom() - 14.0;

        let last = self.shared.store.history().first().map(|m| (*m).clone());
        if let Some(manga) = last {
            let subtitle = tf("Continue · {chapter}", &[("chapter", &manga.progress_line())]);
            if compact {
                let cover = Rect::from_min_max(pos2(column - 21.0, bottom - 63.0), pos2(column + 21.0, bottom));
                let response = ui.interact(cover, Id::new("rail-continue"), Sense::click());
                let hover = ctx.animate_bool_with_time(response.id, response.hovered(), 0.15);
                widgets::paint_cover(ui, &mut self.shared.images, &manga.manga_id, cover, 8, true);
                if hover > 0.0 {
                    painter.rect_stroke(cover, CornerRadius::same(8), Stroke::new(1.5, palette.accent.gamma_multiply(hover)), StrokeKind::Inside);
                }
                rail_flyout(&ctx, cover.center(), rect.right(), hover, &manga.title, Some(&subtitle), palette);
                if response.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                    self.shared.router.resume(&manga);
                }
                bottom = cover.top() - 12.0;
            } else {
                let card = Rect::from_min_max(pos2(row_left, bottom - 78.0), pos2(row_right, bottom));
                let response = ui.interact(card, Id::new("rail-continue"), Sense::click());
                let hover = ctx.animate_bool_with_time(response.id, response.hovered(), 0.15);
                let lift = 2.0 * hover;
                let card = card.translate(vec2(0.0, -lift));
                let shadow = if palette.dark { 60.0 } else { 18.0 } * (0.4 + 0.6 * hover);
                theme::soft_shadow(&painter, card, 14, 4.0 + lift, 16.0, Color32::from_black_alpha(shadow as u8));
                let fill = if palette.dark { 0.7 } else { 0.95 };
                painter.rect_filled(card, CornerRadius::same(14), palette.glass_strong.gamma_multiply(fill * labels));
                let edge = widgets::lerp_color(palette.glass_stroke, palette.accent.gamma_multiply(0.7), hover);
                painter.rect_stroke(card, CornerRadius::same(14), Stroke::new(1.0, edge), StrokeKind::Inside);
                let cover = Rect::from_min_size(card.min + vec2(9.0, 9.0), vec2(40.0, 60.0));
                widgets::paint_cover(ui, &mut self.shared.images, &manga.manga_id, cover, 7, true);
                let text_x = cover.right() + 11.0;
                let room = (card.right() - text_x - 10.0).max(10.0);
                let clip = painter.with_clip_rect(card);
                widgets::paint_line(&clip, pos2(text_x, card.top() + 11.0), &manga.title, theme::bold_font(13.5), palette.text.gamma_multiply(labels), room);
                widgets::paint_line(&clip, pos2(text_x, card.top() + 31.0), &manga.last_chapter_name, theme::font(12.0), palette.secondary.gamma_multiply(labels), room);
                if manga.last_page_count > 0 {
                    let fraction = (manga.last_page + 1) as f32 / manga.last_page_count as f32;
                    let bar = Rect::from_min_size(pos2(text_x, cover.bottom() - 6.0), vec2(room, 4.0));
                    clip.rect_filled(bar, CornerRadius::same(255), palette.quaternary);
                    let filled = Rect::from_min_size(bar.min, vec2(bar.width() * fraction.clamp(0.04, 1.0), bar.height()));
                    clip.rect_filled(filled, CornerRadius::same(255), palette.accent);
                }
                let heading = t("Continue reading").to_uppercase();
                painter.text(pos2(row_left + 4.0, card.top() - 9.0), Align2::LEFT_BOTTOM, heading, theme::bold_font(10.5), palette.tertiary.gamma_multiply(labels));
                if response.on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text(subtitle).clicked() {
                    self.shared.router.resume(&manga);
                }
                bottom = card.top() - 34.0;
            }
        }

        if !net::is_online() {
            let center_y = bottom - 20.0;
            let row = Rect::from_min_max(pos2(row_left, center_y - 18.0), pos2(row_right, center_y + 18.0));
            let response = ui.interact(row, Id::new("rail-offline"), Sense::hover());
            let hover = ctx.animate_bool_with_time(response.id, response.hovered(), 0.12);
            painter.text(pos2(column, center_y), Align2::CENTER_CENTER, icon::WIFI_SLASH, theme::icon_font(19.0), palette.secondary);
            let text = if net::offline_forced() { t("Offline mode") } else { t("You're offline") };
            if labels > 0.0 {
                widgets::paint_line(&painter, pos2(label_x, center_y - 9.0), text, theme::font(13.5), palette.secondary.gamma_multiply(labels), label_room);
            }
            rail_flyout(&ctx, pos2(column, center_y), rect.right(), hover, text, Some(t("Your downloads still work")), palette);
        }
    }

    fn overlays(&mut self, ctx: &Context, palette: &Palette) {
        let elapsed = self.started.elapsed().as_secs_f32();
        if !self.splash_done {
            let appear = (elapsed / 0.55).min(1.0);
            let fade = ((elapsed - 1.25) / 0.35).clamp(0.0, 1.0);
            if fade >= 1.0 {
                self.splash_done = true;
                if !net::is_online() {
                    self.shared.router.show_downloads();
                    self.shared.router.toast_offline();
                }
                if self.locked {
                    self.start_unlock(ctx);
                }
            } else {
                ctx.request_repaint();
                egui::Area::new(Id::new("splash")).order(egui::Order::Debug).fixed_pos(pos2(0.0, 0.0)).show(ctx, |ui| {
                    let rect = ctx.content_rect();
                    let alpha = 1.0 - fade;
                    ui.painter().rect_filled(rect, 0.0, palette.background.gamma_multiply(alpha));
                    theme::radial_glow(ui.painter(), rect.center(), 320.0, palette.accent.gamma_multiply(0.25 * alpha));
                    let ease = theme::ease_out_cubic(appear);
                    let size = 92.0 * (0.9 + 0.1 * ease);
                    ui.painter().text(rect.center(), Align2::CENTER_CENTER, "読", theme::heavy_font(size), palette.accent.gamma_multiply(ease * alpha));
                    ui.allocate_rect(rect, Sense::click());
                });
                return;
            }
        }

        // the lock screen covers everything until windows hello succeeds
        if let Some(promise) = &self.unlocking {
            if let Some(&ok) = promise.ready() {
                if ok {
                    self.locked = false;
                }
                self.unlocking = None;
            }
        }
        if self.locked {
            let mut unlock = false;
            egui::Area::new(Id::new("lock")).order(egui::Order::Debug).fixed_pos(pos2(0.0, 0.0)).show(ctx, |ui| {
                let rect = ctx.content_rect();
                ui.painter().rect_filled(rect, 0.0, palette.background);
                theme::radial_glow(ui.painter(), rect.center() - vec2(0.0, 60.0), 420.0, palette.accent.gamma_multiply(0.3));
                ui.allocate_rect(rect, Sense::click());
                let mut child = ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_center_size(rect.center(), vec2(360.0, 320.0))));
                child.vertical_centered(|ui| {
                    ui.label(RichText::new("読").font(theme::heavy_font(80.0)).color(palette.accent));
                    ui.add_space(8.0);
                    ui.label(RichText::new(t("Yomikaze is locked")).font(theme::bold_font(22.0)));
                    ui.label(RichText::new(t("Unlock with Windows Hello: face, fingerprint or PIN.")).color(palette.secondary));
                    ui.add_space(18.0);
                    if self.unlocking.is_some() {
                        ui.add(egui::Spinner::new().size(26.0));
                    } else if widgets::primary_button(ui, t("Unlock"), Some(icon::LOCK_KEY_OPEN), Some(180.0), true).clicked() {
                        unlock = true;
                    }
                });
            });
            if unlock {
                self.start_unlock(ctx);
            }
            return;
        }

        if self.show_changelog {
            let modal = egui::Modal::new(Id::new("changelog-modal"))
                .backdrop_color(Color32::from_black_alpha(140))
                .frame(theme::modal_frame(palette))
                .show(ctx, |ui| {
                    ui.set_width(520.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(t("What's New")).font(theme::heavy_font(26.0)));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if widgets::primary_button(ui, t("Done"), None, Some(96.0), true).clicked() {
                                ui.close();
                            }
                        });
                    });
                    ui.add_space(8.0);
                    egui::ScrollArea::vertical().max_height(520.0).show(ui, |ui| {
                        settings::changelog_list(ui, palette);
                    });
                });
            if modal.should_close() {
                self.show_changelog = false;
                self.shared.store.settings.last_seen_version = VERSION_LABEL.to_string();
                self.shared.store.settings_changed();
            }
        }

        // a series was just finished: offer to rate it on anilist
        if let Some((title, id)) = self.anilist_finished.first().cloned() {
            let mut done = false;
            let modal = egui::Modal::new(Id::new("anilist-finished"))
                .backdrop_color(Color32::from_black_alpha(140))
                .frame(theme::modal_frame(palette))
                .show(ctx, |ui| {
                    ui.set_width(440.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(icon::CONFETTI).size(30.0).color(palette.accent));
                        ui.vertical(|ui| {
                            ui.label(RichText::new(tf("You finished {title}", &[("title", &title)])).font(theme::bold_font(20.0)));
                            ui.add(egui::Label::new(RichText::new(t("It's marked as Completed on your AniList. Want to give it a score?")).color(palette.secondary)).wrap());
                        });
                    });
                    ui.add_space(16.0);
                    ui.horizontal_wrapped(|ui| {
                        if widgets::primary_button(ui, t("Rate on AniList"), Some(icon::STAR), None, true).clicked() {
                            let _ = open::that(anilist::manga_url(id));
                            done = true;
                        }
                        if widgets::glass_button(ui, t("Not now"), None, None).clicked() {
                            done = true;
                        }
                    });
                });
            if done || modal.should_close() {
                self.anilist_finished.remove(0);
            }
        }

        // the toast centres over the content, not over the sidebar
        let toast_shift = if self.shared.router.reader.is_some() { 0.0 } else { self.sidebar_width / 2.0 };
        if let Some((text, at, glyph)) = self.shared.router.toast.clone() {
            let age = at.elapsed().as_secs_f32();
            if age > 3.5 {
                self.shared.router.toast = None;
            } else {
                ctx.request_repaint();
                let shown = (age / 0.3).min(1.0) * ((3.5 - age) / 0.3).clamp(0.0, 1.0);
                egui::Area::new(Id::new("toast"))
                    .order(egui::Order::Tooltip)
                    .anchor(Align2::CENTER_TOP, vec2(toast_shift, -40.0 + 56.0 * shown))
                    .interactable(false)
                    .show(ctx, |ui| {
                        egui::Frame::new()
                            .fill(palette.glass_strong)
                            .stroke(Stroke::new(1.0, palette.glass_stroke))
                            .corner_radius(CornerRadius::same(255))
                            .inner_margin(egui::Margin::symmetric(20, 11))
                            .show(ui, |ui| {
                                ui.set_opacity(shown);
                                ui.label(RichText::new(format!("{glyph}  {text}")).font(theme::bold_font(14.5)));
                            });
                    });
            }
        }
    }
}

// wallpaper covering rect, zoomed and shifted the way the user set it up
pub fn paint_wallpaper(ui: &Ui, painter: &egui::Painter, texture: egui::TextureId, image_size: egui::Vec2, rect: Rect, zoom: f32, ox: f32, oy: f32) {
    let fill = (rect.width() / image_size.x).max(rect.height() / image_size.y) * zoom.max(1.0);
    let size = image_size * fill;
    let overflow = (size - rect.size()).max(egui::Vec2::ZERO);
    let center = rect.center() + vec2(ox * overflow.x / 2.0, oy * overflow.y / 2.0);
    let image_rect = Rect::from_center_size(center, size);
    let _ = ui;
    painter.image(texture, image_rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
}

// "yomi" in the text colour and "kaze" (wind) in the accent, with a wind stroke under it
// and the japanese spelling next to the version below
fn wordmark(painter: &egui::Painter, pos: Pos2, palette: &Palette, alpha: f32) {
    use egui::text::{LayoutJob, TextFormat};
    let kaze_end = theme::mix(palette.accent, Color32::WHITE, if palette.dark { 0.35 } else { 0.0 });
    let kaze_start = if palette.dark { palette.accent } else { theme::mix(palette.accent, Color32::BLACK, 0.15) };
    let mut job = LayoutJob::default();
    for (index, letter) in "Yomikaze".chars().enumerate() {
        let color = if index < 4 { palette.text } else { theme::mix(kaze_start, kaze_end, (index - 4) as f32 / 3.0) };
        let format = TextFormat { font_id: theme::heavy_font(19.0), color: color.gamma_multiply(alpha), extra_letter_spacing: 0.4, ..Default::default() };
        job.append(&letter.to_string(), 0.0, format);
    }
    let galley = painter.layout_job(job);
    let size = galley.size();
    // where "kaze" starts, for the wind stroke under it
    let kaze_x = galley.rows.first().and_then(|row| row.glyphs.get(4)).map_or(size.x * 0.5, |g| g.pos.x);
    painter.galley(pos, galley, palette.text);

    let base = pos.y + size.y - 3.0;
    let start = pos2(pos.x + kaze_x - 2.0, base + 1.0);
    let end = pos2(pos.x + size.x + 8.0, base - 5.0);
    let control = pos2((start.x + end.x) / 2.0, base + 6.0);
    let points: Vec<Pos2> = (0..=20)
        .map(|i| {
            let t = i as f32 / 20.0;
            let a = start.lerp(control, t);
            let b = control.lerp(end, t);
            a.lerp(b, t)
        })
        .collect();
    // thicker in the middle and fading at both ends, like a brush stroke
    for pair in points.windows(2).enumerate() {
        let (i, segment) = pair;
        let t = i as f32 / 19.0;
        let weight = (t * std::f32::consts::PI).sin();
        let color = theme::mix(kaze_start, kaze_end, t).gamma_multiply(alpha * (0.25 + 0.75 * weight));
        painter.line_segment([segment[0], segment[1]], Stroke::new(0.8 + 1.6 * weight, color));
    }

    let small = pos2(pos.x + 1.0, pos.y + size.y + 3.0);
    let kana = painter.layout_no_wrap("読み風".to_string(), theme::bold_font(10.5), palette.secondary.gamma_multiply(alpha));
    let kana_width = kana.size().x;
    painter.galley(small, kana, palette.secondary);
    let version = format!("·  v{VERSION_LABEL}");
    painter.text(pos2(small.x + kana_width + 6.0, small.y), Align2::LEFT_TOP, version, theme::font(10.5), palette.tertiary.gamma_multiply(alpha));
}

// hover label next to a rail icon, t is the hover animation (0 to 1)
fn rail_flyout(ctx: &Context, anchor: Pos2, rail_right: f32, t: f32, title: &str, subtitle: Option<&str>, palette: &Palette) {
    if t <= 0.01 {
        return;
    }
    let e = theme::ease_out_cubic(t);
    let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip, Id::new("rail-flyout")));
    let title_galley = painter.layout(title.to_string(), theme::bold_font(14.0), palette.text, 260.0);
    let sub_galley = subtitle.map(|s| painter.layout(s.to_string(), theme::font(12.0), palette.secondary, 260.0));
    let width = title_galley.size().x.max(sub_galley.as_ref().map_or(0.0, |g| g.size().x)) + 28.0;
    let height = title_galley.size().y + sub_galley.as_ref().map_or(0.0, |g| g.size().y + 2.0) + 18.0;
    let left = rail_right + 10.0;
    let rect = Rect::from_min_size(pos2(left, anchor.y - height / 2.0), vec2(width, height));
    let fill = if palette.dark { Color32::from_rgb(34, 31, 50) } else { Color32::from_rgb(252, 252, 255) };
    theme::soft_shadow(&painter, rect, 12, 6.0, 18.0, Color32::from_black_alpha((90.0 * e) as u8));
    painter.rect_filled(rect, CornerRadius::same(12), fill.gamma_multiply(e));
    painter.rect_stroke(rect, CornerRadius::same(12), Stroke::new(1.0, palette.glass_stroke.gamma_multiply(e)), StrokeKind::Inside);
    let tip = pos2(rect.left() - 5.0, anchor.y);
    painter.add(egui::Shape::convex_polygon(
        vec![tip, pos2(rect.left() + 1.0, anchor.y - 6.0), pos2(rect.left() + 1.0, anchor.y + 6.0)],
        fill.gamma_multiply(e),
        Stroke::NONE,
    ));
    let mut y = rect.top() + 9.0;
    let title_height = title_galley.size().y;
    painter.galley(pos2(rect.left() + 14.0, y), title_galley, palette.text.gamma_multiply(e));
    y += title_height + 2.0;
    if let Some(galley) = sub_galley {
        painter.galley(pos2(rect.left() + 14.0, y), galley, palette.secondary.gamma_multiply(e));
    }
}
