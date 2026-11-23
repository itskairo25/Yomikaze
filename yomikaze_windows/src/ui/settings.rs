use egui::{Align2, Color32, CornerRadius, RichText, Sense, Stroke, StrokeKind, Ui, vec2};
use crate::icons as icon;
use poll_promise::Promise;

use super::{page, reader, widgets};
use crate::anilist;
use crate::app::{Route, Shared, VERSION_LABEL, paint_wallpaper, spawn};
use crate::images::{Kind, State, disk, format_bytes};
use crate::store::{self, Appearance};
use crate::theme::{self, ACCENTS, CUSTOM_ACCENT, Palette};
use crate::{backup, hello, net, source, wallpapers};
use crate::i18n::{t, tf};

#[derive(Default)]
pub struct SettingsState {
    sizes: Option<Promise<(u64, u64)>>,
    known_sizes: Option<(u64, u64)>,
    wallpaper: Option<Promise<Result<[u8; 3], String>>>,
    editor_open: bool,
    confirm_delete_downloads: bool,
    lock_check: Option<(bool, Promise<bool>)>,
    pending_import: Option<backup::BackupFile>,
    category: Category,
    // a premade picture to use as soon as it has downloaded
    apply_after: Option<String>,
    // 0 is popular, 1 the famous series, then the genres in wallpapers::ANIME_GENRES
    anime_genre: usize,
    // the series picked inside a genre
    anime_series: usize,
    // where the picture being saved as the background came from
    wallpaper_source: Option<String>,
}

impl SettingsState {
    // for the YOMIKAZE_START=settings:<category> switch
    pub fn starting_at(name: &str) -> Self {
        let category = Category::ALL.into_iter().find(|c| format!("{c:?}").eq_ignore_ascii_case(name)).unwrap_or_default();
        Self { category, ..Default::default() }
    }

    fn refresh_sizes(&mut self, ui: &Ui) {
        self.sizes = Some(spawn(ui.ctx(), || (disk::cache_size(), disk::downloads_size())));
    }
}

fn section(ui: &mut Ui, palette: &Palette, title: &str, footer: Option<&str>, add: impl FnOnce(&mut Ui)) {
    ui.label(RichText::new(title.to_uppercase()).font(theme::bold_font(12.0)).color(palette.secondary));
    ui.add_space(4.0);
    theme::card_frame(palette).show(ui, |ui| {
        ui.set_width(ui.available_width());
        add(ui);
    });
    if let Some(footer) = footer {
        ui.add_space(2.0);
        ui.label(RichText::new(footer).size(12.5).color(palette.tertiary));
    }
    ui.add_space(20.0);
}

fn row(ui: &mut Ui, label: &str, value: &str) {
    let palette = Palette::of(ui.ctx());
    ui.horizontal(|ui| {
        ui.label(label);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(RichText::new(value).color(palette.secondary));
        });
    });
}

pub fn show(ui: &mut Ui, state: &mut SettingsState, shared: &mut Shared) {
    let palette = Palette::of(ui.ctx());
    if state.sizes.is_none() && state.known_sizes.is_none() {
        state.refresh_sizes(ui);
    }
    if let Some(promise) = &state.sizes {
        if let Some(sizes) = promise.ready() {
            state.known_sizes = Some(*sizes);
            state.sizes = None;
        }
    }
    poll_wallpaper(state, shared);
    poll_lock(state, shared);

    // same margins as every other page
    let full = ui.max_rect();
    let width = (full.width() - 72.0).clamp(300.0, 1480.0);
    let margin = ((full.width() - width) / 2.0).max(24.0);
    if width - 278.0 < 520.0 {
        // narrow window: the categories become a row of pills above the content
        page(ui, "settings", |ui| {
            widgets::page_title(ui, t("Settings"));
            ui.horizontal_wrapped(|ui| {
                for category in Category::ALL {
                    if widgets::pill(ui, category.title(), Some(category.glyph()), state.category == category, 0.0).clicked() {
                        state.category = category;
                    }
                }
            });
            ui.add_space(18.0);
            category_content(ui, state, shared, &palette);
        });
    } else {
        // the title and the category list stay put, only the chosen category scrolls
        let left = full.left() + margin;
        let mut header = ui.new_child(egui::UiBuilder::new().max_rect(egui::Rect::from_min_size(egui::pos2(left, full.top() + 26.0), vec2(width, 90.0))));
        widgets::page_title(&mut header, t("Settings"));
        let top = header.min_rect().bottom();

        let list_rect = egui::Rect::from_min_max(egui::pos2(left, top), egui::pos2(left + 250.0, full.bottom()));
        let mut list = ui.new_child(egui::UiBuilder::new().max_rect(list_rect));
        egui::ScrollArea::vertical().id_salt("settings-categories").auto_shrink([false, true]).show(&mut list, |ui| {
            ui.set_width(250.0);
            category_list(ui, state, shared, &palette);
            ui.add_space(20.0);
        });

        let content_left = left + 278.0;
        let content_rect = egui::Rect::from_min_max(egui::pos2(content_left, top), full.max);
        let mut content = ui.new_child(egui::UiBuilder::new().max_rect(content_rect));
        content.set_clip_rect(content_rect);
        egui::ScrollArea::vertical()
            .id_salt(("settings-content", state.category as u8))
            .auto_shrink([false, false])
            .show(&mut content, |ui| {
                ui.set_width((left + width - content_left).min(760.0));
                category_content(ui, state, shared, &palette);
                ui.add_space(40.0);
            });
    }

    wallpaper_editor(ui, state, shared, &palette);
    confirm_delete(ui, state, shared, &palette);
    confirm_import(ui, state, shared, &palette);
}

// the settings are split into categories: a list on the left, the chosen one on the right
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Category {
    #[default]
    Appearance,
    Backgrounds,
    Reader,
    AniList,
    Library,
    Backup,
    Privacy,
    About,
}

impl Category {
    const ALL: [Category; 8] = [
        Category::Appearance,
        Category::Backgrounds,
        Category::Reader,
        Category::AniList,
        Category::Library,
        Category::Backup,
        Category::Privacy,
        Category::About,
    ];

    fn title(self) -> &'static str {
        match self {
            Category::Appearance => t("Appearance"),
            Category::Backgrounds => t("Backgrounds"),
            Category::Reader => t("Reader"),
            Category::AniList => "AniList",
            Category::Library => t("Library and storage"),
            Category::Backup => t("Backup"),
            Category::Privacy => t("Privacy"),
            Category::About => t("About"),
        }
    }

    fn glyph(self) -> &'static str {
        match self {
            Category::Appearance => icon::PALETTE,
            Category::Backgrounds => icon::IMAGE,
            Category::Reader => icon::BOOK_OPEN,
            Category::AniList => icon::ARROWS_CLOCKWISE,
            Category::Library => icon::BOOKS,
            Category::Backup => icon::ARROWS_LEFT_RIGHT,
            Category::Privacy => icon::LOCK_KEY,
            Category::About => icon::INFO,
        }
    }

    fn colour(self) -> Color32 {
        match self {
            Category::Appearance => Color32::from_rgb(175, 82, 222),
            Category::Backgrounds => Color32::from_rgb(255, 45, 85),
            Category::Reader => Color32::from_rgb(255, 149, 0),
            Category::AniList => Color32::from_rgb(2, 169, 255),
            Category::Library => Color32::from_rgb(52, 199, 89),
            Category::Backup => Color32::from_rgb(48, 176, 199),
            Category::Privacy => Color32::from_rgb(88, 86, 214),
            Category::About => Color32::from_rgb(142, 142, 147),
        }
    }
}

fn category_list(ui: &mut Ui, state: &mut SettingsState, shared: &Shared, palette: &Palette) {
    theme::card_frame(palette).inner_margin(egui::Margin::same(8)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        for category in Category::ALL {
            let selected = state.category == category;
            let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 46.0), Sense::click());
            let hover = ui.ctx().animate_bool_with_time(response.id, response.hovered(), 0.12);
            if selected {
                ui.painter().rect_filled(rect, CornerRadius::same(12), palette.accent.gamma_multiply(0.2));
            } else if hover > 0.0 {
                ui.painter().rect_filled(rect, CornerRadius::same(12), palette.quaternary.gamma_multiply(hover));
            }
            let tile = egui::Rect::from_center_size(rect.left_center() + vec2(24.0, 0.0), vec2(30.0, 30.0));
            ui.painter().rect_filled(tile, CornerRadius::same(8), category.colour());
            ui.painter().text(tile.center(), Align2::CENTER_CENTER, category.glyph(), theme::icon_font(16.0), Color32::WHITE);
            let color = if selected { palette.text } else { widgets::lerp_color(palette.secondary, palette.text, hover) };
            widgets::paint_line(ui.painter(), rect.left_center() + vec2(50.0, -10.0), category.title(), theme::bold_font(14.5), color, rect.width() - 60.0);
            // a green dot while anilist is connected
            if category == Category::AniList && shared.store.settings.anilist_user.is_some() {
                ui.painter().circle_filled(rect.right_center() - vec2(14.0, 0.0), 4.0, Color32::from_rgb(52, 199, 89));
            }
            if response.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                state.category = category;
            }
        }
    });
}

fn category_content(ui: &mut Ui, state: &mut SettingsState, shared: &mut Shared, palette: &Palette) {
    match state.category {
        Category::Appearance => appearance(ui, shared, palette),
        Category::Backgrounds => {
            background(ui, state, shared, palette);
            gallery(ui, state, shared, palette);
        }
        Category::Reader => {
            section(ui, palette, t("Reader"), Some(t("The same options are behind the gear button inside the reader.")), |ui| {
                reader::reader_options(ui, shared);
            });
        }
        Category::AniList => anilist_section(ui, shared, palette),
        Category::Library => {
        section(ui, palette, t("Content"), None, |ui| {
            if widgets::toggle_row(ui, &mut shared.store.settings.show_adult, t("Show adult content")) {
                shared.store.settings_changed();
            }
        });

        section(ui, palette, t("Connection"), Some(t("Uses no data: only your downloaded chapters and saved series are shown.")), |ui| {
            let mut offline = shared.store.settings.offline_mode;
            if widgets::toggle_row(ui, &mut offline, &format!("{}  {}", icon::WIFI_SLASH, t("Offline mode"))) {
                shared.store.settings.offline_mode = offline;
                shared.store.settings_changed();
                if offline {
                    shared.router.show_downloads();
                }
            }
            if !net::has_connection() {
                ui.label(RichText::new(t("No connection right now.")).size(12.5).color(palette.secondary));
            }
        });

        section(ui, palette, t("Storage"), Some(t("Your library, reading progress, history and downloads are stored only on this PC.")), |ui| {
            let (cache, downloads) = state.known_sizes.unwrap_or((0, 0));
            row(ui, t("Library"), &tf("{n} series", &[("n", &shared.store.library().len())]));
            row(ui, t("Downloaded chapters"), &format!("{} · {}", shared.store.downloads.len(), format_bytes(downloads)));
            row(ui, t("Image cache"), &format_bytes(cache));
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if widgets::glass_button(ui, t("Clear image cache"), Some(icon::BROOM), None).clicked() {
                    disk::clear_cache();
                    shared.images.clear();
                    state.refresh_sizes(ui);
                }
                if !shared.store.downloads.is_empty() && widgets::glass_button(ui, t("Delete all downloads"), Some(icon::TRASH), None).clicked() {
                    state.confirm_delete_downloads = true;
                }
            });
            ui.add_space(4.0);
            if widgets::link_button(ui, t("Open data folder")).clicked() {
                let _ = open::that(store::data_dir());
            }
        });
        }
        Category::Backup => {
        section(
            ui,
            palette,
            t("Backup"),
            Some(t("Saves your library, categories, history and the page you're on in every chapter to one file. Open it in Yomikaze on your iPhone or another PC to carry on where you left off. Downloaded pages aren't included.")),
            |ui| {
                ui.horizontal_wrapped(|ui| {
                    if widgets::glass_button(ui, t("Export progress..."), Some(icon::EXPORT), None).clicked() {
                        export_backup(shared);
                    }
                    if widgets::glass_button(ui, t("Import..."), Some(icon::DOWNLOAD_SIMPLE), None).clicked() {
                        if let Some(path) = rfd::FileDialog::new().add_filter(t("Yomikaze backup"), &["json"]).pick_file() {
                            match std::fs::read(&path).map_err(|e| e.to_string()).and_then(|bytes| backup::parse(&bytes)) {
                                Ok(file) => state.pending_import = Some(file),
                                Err(error) => shared.router.toast_warning(error),
                            }
                        }
                    }
                });
            },
        );
        }
        Category::Privacy => {
        section(
            ui,
            palette,
            t("Privacy"),
            Some(t("Asks for Windows Hello (face, fingerprint or your PIN) every time the app is opened or restored.")),
            |ui| {
                let mut lock = shared.store.settings.app_lock;
                let checking = state.lock_check.is_some();
                ui.add_enabled_ui(!checking, |ui| {
                    if widgets::toggle_row(ui, &mut lock, &format!("{}  {}", icon::LOCK_KEY, t("Lock with Windows Hello"))) {
                        // turning the lock on or off asks windows hello itself
                        let hwnd = shared.hwnd;
                        let message = if lock { t("Turn on the app lock") } else { t("Turn off the app lock") };
                        state.lock_check = Some((lock, spawn(ui.ctx(), move || hello::verify(hwnd, message))));
                    }
                });
                if checking {
                    ui.label(RichText::new(t("Waiting for Windows Hello...")).size(12.5).color(palette.secondary));
                }
            },
        );
        }
        Category::About => {
        section(ui, palette, t("About"), None, |ui| {
            if nav_row(ui, icon::SPARKLE, t("What's New")) {
                shared.router.push(Route::Changelog);
            }
            if nav_row(ui, icon::USERS_THREE, t("Credits")) {
                shared.router.push(Route::Credits);
            }
            ui.add_space(4.0);
            row(ui, t("Version"), VERSION_LABEL);
            row(ui, t("Source"), "WeebCentral");
            if widgets::link_button(ui, t("Open weebcentral.com")).clicked() {
                let _ = open::that(source::BASE);
            }
        });
        }
    }
}

fn nav_row(ui: &mut Ui, glyph: &str, label: &str) -> bool {
    let palette = Palette::of(ui.ctx());
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 40.0), Sense::click());
    if response.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::same(10), palette.quaternary);
    }
    ui.painter().text(rect.left_center() + vec2(8.0, 0.0), Align2::LEFT_CENTER, glyph, theme::icon_font(18.0), palette.accent);
    ui.painter().text(rect.left_center() + vec2(36.0, 0.0), Align2::LEFT_CENTER, label, theme::font(15.0), palette.text);
    ui.painter().text(rect.right_center() - vec2(8.0, 0.0), Align2::RIGHT_CENTER, icon::CARET_RIGHT, theme::icon_font(15.0), palette.tertiary);
    response.on_hover_cursor(egui::CursorIcon::PointingHand).clicked()
}

fn anilist_section(ui: &mut Ui, shared: &mut Shared, palette: &Palette) {
    let footer = t("Syncs your manga list, favourites and reading progress with AniList. Nothing is ever deleted from your AniList.");
    section(ui, palette, "AniList", Some(footer), |ui| {
        if !anilist::configured() {
            ui.label(RichText::new(t("AniList isn't set up in this build.")).color(palette.secondary));
            return;
        }
        let status = shared.anilist.status();
        let user = shared.store.settings.anilist_user.clone();
        if let (Some(user), true) = (user, shared.anilist_token.is_some()) {
            row(ui, t("Account"), &user.name);
            let last = shared.store.settings.anilist_last_sync;
            let state = if status.busy && status.message.contains('/') {
                tf("Importing {progress}...", &[("progress", &status.message)])
            } else if status.busy {
                t("Syncing...").to_string()
            } else if let Some(error) = &status.error {
                error.clone()
            } else if last > 0 {
                tf("Last synced {when}", &[("when", &crate::model::relative_time(last))])
            } else {
                t("Not synced yet").to_string()
            };
            row(ui, t("Status"), &state);
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if widgets::glass_button(ui, t("Sync now"), Some(icon::ARROWS_CLOCKWISE), None).clicked() && !status.busy {
                    shared.anilist_sync_request = Some(true);
                }
                if widgets::glass_button(ui, t("Disconnect"), Some(icon::SIGN_OUT), None).clicked() {
                    anilist::delete_token();
                    shared.anilist.clear();
                    shared.anilist_token = None;
                    shared.store.settings.anilist_user = None;
                    shared.store.settings_changed();
                }
            });
        } else if shared.anilist_waiting {
            ui.label(RichText::new(t("Finish the login in your browser, Yomikaze picks it up by itself.")).color(palette.secondary));
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if widgets::glass_button(ui, t("Open the login page again"), Some(icon::ARROW_SQUARE_OUT), None).clicked() {
                    let _ = open::that(anilist::login_url());
                }
                if widgets::link_button(ui, t("Cancel")).clicked() {
                    shared.anilist_waiting = false;
                }
            });
        } else if widgets::primary_button(ui, t("Connect AniList"), Some(icon::LINK), None, true).clicked() {
            crate::hello::register_link_protocol();
            let _ = open::that(anilist::login_url());
            shared.anilist_waiting = true;
        }
    });
}

fn appearance(ui: &mut Ui, shared: &mut Shared, palette: &Palette) {
    section(ui, palette, t("Appearance"), None, |ui| {
        let mut changed = false;
        ui.label(RichText::new(t("Theme")).font(theme::bold_font(14.5)));
        ui.horizontal(|ui| {
            for (value, label, glyph) in [
                (Appearance::System, t("System"), icon::DESKTOP),
                (Appearance::Light, t("Light"), icon::SUN),
                (Appearance::Dark, t("Dark"), icon::MOON),
            ] {
                if widgets::pill(ui, label, Some(glyph), shared.store.settings.appearance == value, 110.0).clicked() {
                    shared.store.settings.appearance = value;
                    changed = true;
                }
            }
        });
        ui.add_space(12.0);

        ui.label(RichText::new(t("Language")).font(theme::bold_font(14.5)));
        ui.horizontal_wrapped(|ui| {
            for (code, name) in crate::i18n::LANGUAGES {
                let label = if code == "system" { t(name) } else { name };
                if widgets::pill(ui, label, None, shared.store.settings.language == code, 0.0).clicked() {
                    shared.store.settings.language = code.to_string();
                    changed = true;
                }
            }
        });
        ui.add_space(12.0);

        ui.label(RichText::new(t("Accent colour")).font(theme::bold_font(14.5)));
        ui.horizontal(|ui| {
            for (id, name, color) in ACCENTS {
                let selected = shared.store.settings.accent == id;
                let (rect, response) = ui.allocate_exact_size(vec2(62.0, 64.0), Sense::click());
                let center = rect.center_top() + vec2(0.0, 22.0);
                ui.painter().circle_filled(center, 19.0, color);
                if selected {
                    ui.painter().circle_stroke(center, 23.0, Stroke::new(2.0, color));
                    ui.painter().text(center, Align2::CENTER_CENTER, icon::CHECK, theme::icon_font(16.0), Color32::WHITE);
                }
                ui.painter().text(rect.center_bottom() - vec2(0.0, 6.0), Align2::CENTER_CENTER, name, theme::font(11.5), palette.secondary);
                if response.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                    shared.store.settings.accent = id.to_string();
                    changed = true;
                }
            }
        });
        ui.horizontal(|ui| {
            let mut rgb = shared.store.settings.custom_accent;
            if ui.color_edit_button_srgb(&mut rgb).changed() {
                shared.store.settings.custom_accent = rgb;
                shared.store.settings.accent = CUSTOM_ACCENT.to_string();
                changed = true;
            }
            ui.label(t("Custom colour"));
            if shared.store.settings.accent == CUSTOM_ACCENT {
                ui.label(RichText::new(icon::CHECK).color(palette.accent));
            }
        });
        ui.add_space(12.0);

        ui.label(RichText::new(t("Interface size")).font(theme::bold_font(14.5)));
        let mut scale = shared.store.settings.ui_scale;
        let response = ui
            .horizontal(|ui| {
                let response = widgets::slider(ui, &mut scale, 0.8..=1.4, Some(0.05), false);
                ui.label(RichText::new(format!("{:.0}%", scale * 100.0)).font(theme::bold_font(14.0)).color(palette.secondary));
                response
            })
            .inner;
        if response.drag_stopped() || (response.changed() && !response.dragged()) {
            shared.store.settings.ui_scale = scale;
            changed = true;
        } else if response.changed() {
            shared.store.settings.ui_scale = scale;
        }

        if changed {
            shared.store.settings_changed();
            shared.router.theme_dirty = true;
        }
    });
}

fn background(ui: &mut Ui, state: &mut SettingsState, shared: &mut Shared, palette: &Palette) {
    section(
        ui,
        palette,
        t("Background"),
        Some(t("Shown behind Library, Browse, Search, History and Settings. The accent colour is matched to the picture when you pick one.")),
        |ui| {
            let has = shared.store.settings.wallpaper_rev > 0;
            ui.horizontal(|ui| {
                let label = if has { t("Change background image") } else { t("Choose background image") };
                if widgets::glass_button(ui, label, Some(icon::IMAGE), None).clicked() && state.wallpaper.is_none() {
                    if let Some(path) = rfd::FileDialog::new().add_filter(t("Images"), &["png", "jpg", "jpeg", "webp", "gif", "bmp"]).pick_file() {
                        state.wallpaper = Some(spawn(ui.ctx(), move || save_wallpaper(&path)));
                        state.wallpaper_source = None;
                    }
                }
                if state.wallpaper.is_some() {
                    ui.add(egui::Spinner::new());
                }
            });
            if !has {
                return;
            }
            ui.add_space(8.0);
            let url = store::wallpaper_url(shared.store.settings.wallpaper_rev);
            let width = ui.available_width();
            let (rect, _) = ui.allocate_exact_size(vec2(width, 130.0), Sense::hover());
            if let State::Ready(loaded) = shared.images.get(&url, Kind::Wide) {
                let s = &shared.store.settings;
                let painter = ui.painter_at(rect);
                paint_wallpaper(ui, &painter, loaded.first().id(), loaded.size, rect, s.wallpaper_zoom, s.wallpaper_x, s.wallpaper_y);
            }
            ui.painter().rect_stroke(rect, CornerRadius::same(12), Stroke::new(1.0, palette.glass_stroke), StrokeKind::Inside);
            ui.add_space(8.0);
            if widgets::glass_button(ui, t("Adjust position and zoom"), Some(icon::CROP), None).clicked() {
                state.editor_open = true;
            }
            ui.add_space(6.0);
            ui.label(t("Dimming"));
            if widgets::slider(ui, &mut shared.store.settings.wallpaper_dim, 0.2..=0.9, None, false).changed() {
                shared.store.settings_changed();
            }
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if widgets::link_button(ui, &format!("{}  {}", icon::EYEDROPPER, t("Match accent colour to image"))).clicked() {
                    if let Ok(bytes) = std::fs::read(store::wallpaper_path()) {
                        if let Some(rgb) = image::load_from_memory(&bytes).ok().map(|img| matching_accent(&img)) {
                            shared.store.settings.custom_accent = rgb;
                            shared.store.settings.accent = CUSTOM_ACCENT.to_string();
                            shared.store.settings_changed();
                            shared.router.theme_dirty = true;
                        }
                    }
                }
            });
            if widgets::link_button(ui, &format!("{}  {}", icon::TRASH, t("Remove background image"))).clicked() {
                shared.images.forget(&url);
                let _ = std::fs::remove_file(store::wallpaper_path());
                shared.store.settings.wallpaper_rev = 0;
                shared.store.settings.wallpaper_source.clear();
                shared.store.settings_changed();
            }
        },
    );
}

// anime wallpapers by genre and series. previews only until one is picked, then the full
// picture is saved on the pc.
fn gallery(ui: &mut Ui, state: &mut SettingsState, shared: &mut Shared, palette: &Palette) {
    for event in shared.gallery.poll() {
        match event {
            wallpapers::Event::Saved(url) => {
                if state.apply_after.as_deref() == Some(url.as_str()) {
                    state.apply_after = None;
                    apply_wallpaper(ui, state, &url);
                }
            }
            wallpapers::Event::Failed(url, error) => {
                if state.apply_after.as_deref() == Some(url.as_str()) {
                    state.apply_after = None;
                    shared.router.toast_warning(tf("Couldn't download that picture: {error}", &[("error", &error)]));
                }
            }
        }
    }
    let current = if shared.store.settings.wallpaper_rev > 0 { shared.store.settings.wallpaper_source.clone() } else { String::new() };
    let mut clicked: Option<String> = None;

    let footer = t("Anime wallpapers from Wallhaven, safe for work and at least Full HD. Only the one you pick is downloaded to your PC, so it works offline too.");
    section(ui, palette, t("Anime backgrounds"), Some(footer), |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
            let names = ["Popular", "Famous series"].into_iter().chain(wallpapers::ANIME_GENRES);
            for (index, name) in names.enumerate() {
                if widgets::pill(ui, t(name), None, state.anime_genre == index, 0.0).clicked() && state.anime_genre != index {
                    state.anime_genre = index;
                    state.anime_series = 0;
                }
            }
        });
        ui.add_space(12.0);

        // popular is one list, famous and the genres first list their series
        let query = if state.anime_genre == 0 {
            String::new()
        } else {
            let mut titles: Vec<String> = Vec::new();
            if state.anime_genre == 1 {
                titles = wallpapers::FAMOUS_SERIES.iter().map(|s| s.to_string()).collect();
            } else if let Some(set) = shared.gallery.anime().and_then(|s| s.sets.get(state.anime_genre - 1)) {
                for item in set {
                    let name = wallpapers::search_name(&item.title);
                    if !titles.contains(&name) && titles.len() < 24 {
                        titles.push(name);
                    }
                }
            }
            if titles.is_empty() {
                if shared.gallery.anime_loading() {
                    widgets::spinner(ui);
                } else {
                    let message = shared.gallery.anime_error().map(str::to_string).unwrap_or_else(|| t("You're offline").to_string());
                    ui.label(RichText::new(message).color(palette.secondary));
                    if net::is_online() && widgets::glass_button(ui, t("Try again"), Some(icon::ARROWS_CLOCKWISE), None).clicked() {
                        shared.gallery.retry_anime();
                    }
                }
                return;
            }
            let chosen = state.anime_series.min(titles.len() - 1);
            ui.label(RichText::new(t("Series")).font(theme::bold_font(13.0)).color(palette.secondary));
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
                for (index, title) in titles.iter().enumerate() {
                    if widgets::chip_button(ui, title, index == chosen).clicked() {
                        state.anime_series = index;
                    }
                }
            });
            ui.add_space(14.0);
            titles[chosen].clone()
        };

        let search = shared.gallery.wallpapers(&query);
        let (items, loading, error, more, searched) = (search.items.clone(), search.loading(), search.error.clone(), search.has_more(), search.page > 0);
        super::grid(ui, items.len(), 200.0, 12.0, |ui, index, width| {
            let item = &items[index];
            let tip = format!("{} · Wallhaven", item.resolution);
            if background_tile(ui, shared, palette, &item.full, &item.thumb, "", tip, width, 0.5625, current == item.full) {
                clicked = Some(item.full.clone());
            }
        });
        if loading {
            widgets::spinner(ui);
        } else if let Some(error) = error {
            ui.label(RichText::new(error).color(palette.secondary));
            if net::is_online() && widgets::glass_button(ui, t("Try again"), Some(icon::ARROWS_CLOCKWISE), None).clicked() {
                shared.gallery.retry_wallpapers(&query);
            }
        } else if searched && items.is_empty() {
            ui.label(RichText::new(t("Nothing found")).color(palette.secondary));
        } else if !searched && !net::is_online() {
            ui.label(RichText::new(t("You're offline")).color(palette.secondary));
        } else if more {
            ui.vertical_centered(|ui| {
                if widgets::glass_button(ui, t("Load more"), Some(icon::CARET_DOWN), Some(180.0)).clicked() {
                    shared.gallery.more_wallpapers(&query);
                }
            });
        }
    });

    if let Some(url) = clicked {
        if shared.gallery.is_saved(&url) {
            apply_wallpaper(ui, state, &url);
        } else if net::is_online() {
            state.apply_after = Some(url.clone());
            shared.gallery.download(&url);
        } else {
            shared.router.toast_offline();
        }
    }
}

// one picture in the gallery, returns true when it's clicked
#[allow(clippy::too_many_arguments)]
fn background_tile(ui: &mut Ui, shared: &mut Shared, palette: &Palette, url: &str, preview: &str, name: &str, tip: String, width: f32, ratio: f32, selected: bool) -> bool {
    let (rect, response) = ui.allocate_exact_size(vec2(width, (width * ratio).round()), Sense::click());
    // pictures further down load once they're scrolled into view
    if !ui.is_rect_visible(rect.expand(200.0)) {
        ui.painter().rect_filled(rect, CornerRadius::same(12), palette.quaternary);
        return false;
    }
    let hover = ui.ctx().animate_bool_with_time(response.id, response.hovered(), 0.16);
    let rect = rect.translate(vec2(0.0, -3.0 * theme::ease_out_cubic(hover)));
    if hover > 0.0 {
        theme::soft_shadow(ui.painter(), rect, 12, 6.0, 18.0, Color32::from_black_alpha((70.0 * hover) as u8));
    }
    let saved = shared.gallery.is_saved(url);
    // the saved picture if it's on disk, otherwise the preview
    let source = if saved { wallpapers::local_url(url) } else { preview.to_string() };
    let image = shared.images.get(&source, Kind::Cover);
    let ready = matches!(image, State::Ready(_));
    let failed = matches!(image, State::Failed);
    let fade = ui.ctx().animate_bool_with_time(egui::Id::new(("tile-fade", url)), ready, 0.3);
    if fade < 1.0 {
        ui.painter().rect_filled(rect, CornerRadius::same(12), palette.quaternary);
    }
    widgets::paint_image(ui, image, rect, 12, Color32::WHITE.gamma_multiply(fade));
    if failed {
        // try a failed preview again now and then, the server may only have been busy
        let retry_id = egui::Id::new(("tile-retry", url));
        let now = ui.input(|i| i.time);
        let last = ui.ctx().data(|d| d.get_temp::<f64>(retry_id));
        if last.is_none_or(|t| now - t > 30.0) && net::is_online() {
            ui.ctx().data_mut(|d| d.insert_temp(retry_id, now));
            if last.is_some() {
                shared.images.retry(&source, Kind::Cover);
            }
        }
        let glyph = if net::is_online() { icon::IMAGE } else { icon::WIFI_SLASH };
        ui.painter().text(rect.center() - vec2(0.0, 8.0), Align2::CENTER_CENTER, glyph, theme::icon_font(22.0), palette.tertiary);
    }
    if shared.gallery.is_busy(url) {
        ui.painter().rect_filled(rect, CornerRadius::same(12), Color32::from_black_alpha(110));
        let time = ui.input(|i| i.time) as f32;
        for k in 0..3 {
            let phase = (time * 3.2 - k as f32 * 0.55).sin() * 0.5 + 0.5;
            let dot = rect.center() + vec2((k as f32 - 1.0) * 14.0, -8.0 - 3.0 * phase);
            ui.painter().circle_filled(dot, 3.5, Color32::WHITE.gamma_multiply(0.4 + 0.6 * phase));
        }
        ui.ctx().request_repaint();
    }
    if !name.is_empty() {
        let band = egui::Rect::from_min_max(egui::pos2(rect.left(), rect.bottom() - 34.0), rect.max);
        let painter = ui.painter().with_clip_rect(rect.shrink(1.0));
        theme::vertical_gradient(&painter, band, Color32::TRANSPARENT, Color32::from_black_alpha(170));
        widgets::paint_line(&painter, egui::pos2(rect.left() + 10.0, rect.bottom() - 23.0), name, theme::bold_font(12.5), Color32::WHITE, rect.width() - 20.0);
    }
    let edge = if selected { egui::Stroke::new(2.5, palette.accent) } else { egui::Stroke::new(1.0, palette.glass_stroke) };
    ui.painter().rect_stroke(rect, CornerRadius::same(12), edge, StrokeKind::Inside);
    if selected {
        let badge = rect.right_top() + vec2(-15.0, 15.0);
        ui.painter().circle_filled(badge, 10.0, palette.accent);
        ui.painter().text(badge, Align2::CENTER_CENTER, icon::CHECK, theme::icon_font(13.0), theme::readable_on(palette.accent));
    }
    let tip = if !saved && !net::is_online() { format!("{tip}\n{}", t("Not downloaded yet")) } else { tip };
    response.on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text(tip).clicked()
}

fn apply_wallpaper(ui: &Ui, state: &mut SettingsState, url: &str) {
    if state.wallpaper.is_some() {
        return;
    }
    let path = wallpapers::file_for(url);
    state.wallpaper = Some(spawn(ui.ctx(), move || save_wallpaper(&path)));
    state.wallpaper_source = Some(url.to_string());
}

// scaled down and saved off the ui thread, returns a matching accent
fn save_wallpaper(path: &std::path::Path) -> Result<[u8; 3], String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let image = image::load_from_memory(&bytes).map_err(|e| e.to_string())?;
    let image = if image.width() > 3840 { image.resize(3840, 3840 * 4, image::imageops::FilterType::Lanczos3) } else { image };
    image.to_rgba8().save(store::wallpaper_path()).map_err(|e| e.to_string())?;
    Ok(matching_accent(&image))
}

// average colour pushed towards something vivid enough for an accent
fn matching_accent(image: &image::DynamicImage) -> [u8; 3] {
    let tiny = image.resize_exact(1, 1, image::imageops::FilterType::Triangle).to_rgb8();
    let p = tiny.get_pixel(0, 0);
    let color = egui::Rgba::from_srgba_unmultiplied(p[0], p[1], p[2], 255);
    let mut hsva = egui::ecolor::Hsva::from(color);
    hsva.s = (hsva.s * 1.6).clamp(0.5, 0.85);
    hsva.v = (hsva.v * 1.3).clamp(0.75, 0.98);
    let out = Color32::from(hsva);
    [out.r(), out.g(), out.b()]
}

fn poll_wallpaper(state: &mut SettingsState, shared: &mut Shared) {
    let Some(promise) = &state.wallpaper else { return };
    let Some(result) = promise.ready() else { return };
    match result {
        Ok(rgb) => {
            let settings = &mut shared.store.settings;
            settings.wallpaper_rev += 1;
            settings.wallpaper_zoom = 1.0;
            settings.wallpaper_x = 0.0;
            settings.wallpaper_y = 0.0;
            settings.custom_accent = *rgb;
            settings.accent = CUSTOM_ACCENT.to_string();
            settings.wallpaper_source = state.wallpaper_source.take().unwrap_or_default();
            shared.store.settings_changed();
            shared.router.theme_dirty = true;
        }
        Err(error) => shared.router.toast_warning(tf("Couldn't use that picture: {error}", &[("error", &error)])),
    }
    state.wallpaper = None;
}

fn poll_lock(state: &mut SettingsState, shared: &mut Shared) {
    let Some((wanted, promise)) = &state.lock_check else { return };
    let Some(&ok) = promise.ready() else { return };
    if ok {
        shared.store.settings.app_lock = *wanted;
        shared.store.settings_changed();
    }
    state.lock_check = None;
}

fn wallpaper_editor(ui: &mut Ui, state: &mut SettingsState, shared: &mut Shared, palette: &Palette) {
    if !state.editor_open {
        return;
    }
    let url = store::wallpaper_url(shared.store.settings.wallpaper_rev);
    let modal = egui::Modal::new(egui::Id::new("wallpaper-editor"))
        .frame(theme::modal_frame(palette))
        .show(ui.ctx(), |ui| {
            ui.set_width(720.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new(t("Adjust background")).font(theme::bold_font(22.0)));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if widgets::primary_button(ui, t("Done"), None, Some(96.0), true).clicked() {
                        ui.close();
                    }
                });
            });
            ui.label(RichText::new(t("Drag to move, scroll to zoom")).color(palette.secondary));
            ui.add_space(10.0);

            let size = vec2(ui.available_width(), ui.available_width() * 10.0 / 16.0);
            let (rect, response) = ui.allocate_exact_size(size, Sense::drag());
            let settings = &mut shared.store.settings;
            let mut changed = false;
            if let State::Ready(loaded) = shared.images.get(&url, Kind::Wide) {
                let image_size = loaded.size;
                let painter = ui.painter_at(rect);
                paint_wallpaper(ui, &painter, loaded.first().id(), image_size, rect, settings.wallpaper_zoom, settings.wallpaper_x, settings.wallpaper_y);
                // offsets are a fraction of the overflow so they work at any window size
                let fill = (rect.width() / image_size.x).max(rect.height() / image_size.y) * settings.wallpaper_zoom;
                let overflow = (image_size * fill - rect.size()).max(egui::Vec2::ZERO);
                if response.dragged() {
                    let delta = response.drag_delta();
                    if overflow.x > 0.0 {
                        settings.wallpaper_x = (settings.wallpaper_x + delta.x / (overflow.x / 2.0)).clamp(-1.0, 1.0);
                    }
                    if overflow.y > 0.0 {
                        settings.wallpaper_y = (settings.wallpaper_y + delta.y / (overflow.y / 2.0)).clamp(-1.0, 1.0);
                    }
                    changed = true;
                }
                if response.hovered() {
                    let scroll = ui.input(|i| i.smooth_scroll_delta.y);
                    if scroll != 0.0 {
                        settings.wallpaper_zoom = (settings.wallpaper_zoom * (1.0 + scroll * 0.002)).clamp(1.0, 4.0);
                        changed = true;
                    }
                }
            }
            ui.painter().rect_stroke(rect, CornerRadius::same(18), Stroke::new(1.0, palette.glass_stroke), StrokeKind::Inside);
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new(icon::MAGNIFYING_GLASS_MINUS).color(palette.secondary));
                changed |= widgets::slider(ui, &mut settings.wallpaper_zoom, 1.0..=4.0, None, false).changed();
                ui.label(RichText::new(icon::MAGNIFYING_GLASS_PLUS).color(palette.secondary));
                if widgets::glass_button(ui, t("Reset position"), None, None).clicked() {
                    settings.wallpaper_zoom = 1.0;
                    settings.wallpaper_x = 0.0;
                    settings.wallpaper_y = 0.0;
                    changed = true;
                }
            });
            if changed {
                shared.store.settings_changed();
            }
        });
    if modal.should_close() {
        state.editor_open = false;
    }
}

fn export_backup(shared: &mut Shared) {
    let Some(path) = rfd::FileDialog::new()
        .set_file_name(backup::file_name())
        .add_filter(t("Yomikaze backup"), &["json"])
        .save_file()
    else {
        return;
    };
    let file = backup::export(&shared.store);
    let result = serde_json::to_vec_pretty(&file).map_err(|e| e.to_string()).and_then(|bytes| std::fs::write(&path, bytes).map_err(|e| e.to_string()));
    match result {
        Ok(()) => shared.router.toast(tf("Saved {series} series and {chapters} chapter positions", &[("series", &file.manga.len()), ("chapters", &file.progress.len())])),
        Err(error) => shared.router.toast_warning(tf("Couldn't save the backup: {error}", &[("error", &error)])),
    }
}

fn confirm_import(ui: &mut Ui, state: &mut SettingsState, shared: &mut Shared, palette: &Palette) {
    let Some(file) = state.pending_import.clone() else { return };
    let mut close = false;
    let modal = egui::Modal::new(egui::Id::new("confirm-import"))
        .frame(theme::modal_frame(palette))
        .show(ui.ctx(), |ui| {
            ui.set_width(440.0);
            ui.label(RichText::new(t("Import backup?")).font(theme::bold_font(21.0)));
            let from = match file.app.as_str() {
                "ios" => "iPhone",
                "windows" => "Windows",
                other => other,
            };
            let library = file.manga.iter().filter(|m| m.in_library).count();
            ui.label(
                RichText::new(tf(
                    "From Yomikaze {version} on {from}: {series} series ({library} in the library) and {chapters} chapter positions.",
                    &[
                        ("version", &file.app_version),
                        ("from", &from),
                        ("series", &file.manga.len()),
                        ("library", &library),
                        ("chapters", &file.progress.len()),
                    ],
                ))
                .color(palette.secondary),
            );
            ui.add_space(4.0);
            ui.label(
                RichText::new(t("Nothing is deleted: your library is merged with the backup, and for every chapter the most recent page is kept."))
                    .size(13.0)
                    .color(palette.tertiary),
            );
            ui.add_space(14.0);
            let mut choice: Option<bool> = None;
            ui.horizontal_wrapped(|ui| {
                if widgets::primary_button(ui, t("Import progress"), Some(icon::DOWNLOAD_SIMPLE), None, true).clicked() {
                    choice = Some(false);
                }
                if file.settings.is_some() && widgets::glass_button(ui, t("Progress and settings"), None, None).clicked() {
                    choice = Some(true);
                }
                if widgets::glass_button(ui, t("Cancel"), None, None).clicked() {
                    close = true;
                }
            });
            if let Some(with_settings) = choice {
                let summary = backup::apply(&mut shared.store, &file, with_settings);
                shared.store.flush(true);
                if with_settings {
                    shared.router.theme_dirty = true;
                }
                shared.router.toast(tf("Imported {series} series and {chapters} chapter positions", &[("series", &summary.series), ("chapters", &summary.chapters)]));
                close = true;
            }
        });
    if close || modal.should_close() {
        state.pending_import = None;
    }
}

fn confirm_delete(ui: &mut Ui, state: &mut SettingsState, shared: &mut Shared, palette: &Palette) {
    if !state.confirm_delete_downloads {
        return;
    }
    let mut close = false;
    let modal = egui::Modal::new(egui::Id::new("delete-all-downloads"))
        .frame(theme::modal_frame(palette))
        .show(ui.ctx(), |ui| {
            ui.set_width(380.0);
            ui.label(RichText::new(t("Delete all downloaded chapters?")).font(theme::bold_font(20.0)));
            ui.add_space(14.0);
            ui.horizontal(|ui| {
                if widgets::glass_button(ui, t("Cancel"), None, Some(130.0)).clicked() {
                    close = true;
                }
                if widgets::primary_button(ui, t("Delete downloads"), Some(icon::TRASH), Some(200.0), true).clicked() {
                    shared.downloads.cancel_all();
                    shared.store.downloads.clear();
                    shared.store.mark_dirty();
                    disk::remove_all_downloads();
                    state.refresh_sizes(ui);
                    close = true;
                }
            });
        });
    if close || modal.should_close() {
        state.confirm_delete_downloads = false;
    }
}

const CREDITS: [(&str, &str, &[&str]); 2] = [
    ("Lead Developer", icon::HAMMER, &["Kairo"]),
    ("Early Access Tester", icon::FLASK, &["illusive"]),
];

// kro, the lead developer's picture, decoded once
fn avatar(ctx: &egui::Context) -> Option<egui::TextureHandle> {
    let id = egui::Id::new("credits-avatar");
    if let Some(texture) = ctx.data(|d| d.get_temp::<egui::TextureHandle>(id)) {
        return Some(texture);
    }
    let image = image::load_from_memory(include_bytes!("../../assets/kairo.png")).ok()?.to_rgba8();
    let size = [image.width() as usize, image.height() as usize];
    let texture = ctx.load_texture("credits-avatar", egui::ColorImage::from_rgba_unmultiplied(size, image.as_raw()), egui::TextureOptions::LINEAR);
    ctx.data_mut(|d| d.insert_temp(id, texture.clone()));
    Some(texture)
}

pub fn credits(ui: &mut Ui, shared: &mut Shared) {
    let palette = Palette::of(ui.ctx());
    if ui.input(|i| i.key_pressed(egui::Key::Escape) || i.pointer.button_clicked(egui::PointerButton::Extra1)) {
        shared.router.pop();
        return;
    }
    page(ui, "credits", |ui| {
        if widgets::glass_button(ui, t("Back"), Some(icon::CARET_LEFT), None).clicked() {
            shared.router.pop();
        }
        ui.vertical_centered(|ui| {
            ui.set_max_width(560.0);
            ui.add_space(10.0);
            ui.label(RichText::new("読").font(theme::heavy_font(72.0)).color(palette.accent));
            ui.label(RichText::new("Yomikaze").font(theme::bold_font(26.0)));
            ui.label(RichText::new(t("Made with care by")).color(palette.secondary));
            ui.add_space(20.0);
            for (role, glyph, people) in CREDITS {
                theme::glass_frame(&palette).fill(palette.glass_strong.gamma_multiply(0.75)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new(format!("{glyph}  {}", t(role).to_uppercase())).font(theme::bold_font(12.0)).color(palette.secondary));
                    ui.add_space(8.0);
                    for person in people {
                        ui.horizontal(|ui| {
                            let (rect, _) = ui.allocate_exact_size(vec2(44.0, 44.0), Sense::hover());
                            // the lead developer has a picture, everyone else gets their initial
                            match (*person == "Kairo").then(|| avatar(ui.ctx())).flatten() {
                                Some(texture) => {
                                    egui::Image::new((texture.id(), rect.size())).corner_radius(CornerRadius::same(22)).paint_at(ui, rect);
                                }
                                None => {
                                    ui.painter().circle_filled(rect.center(), 22.0, palette.accent);
                                    let initial = person.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_default();
                                    ui.painter().text(rect.center(), Align2::CENTER_CENTER, initial, theme::bold_font(18.0), theme::readable_on(palette.accent));
                                }
                            }
                            ui.label(RichText::new(*person).font(theme::bold_font(20.0)));
                        });
                    }
                });
                ui.add_space(14.0);
            }
        });
    });
}

pub struct Release {
    pub version: &'static str,
    pub title: &'static str,
    pub changes: &'static [(&'static str, &'static str)],
}

pub const RELEASES: [Release; 4] = [
    Release {
        version: "0.2.7 hotfix",
        title: "Hotfix",
        changes: &[
            (icon::BOOK_OPEN, "Two pages side by side: a page no longer shows up twice in a row when turning pages"),
            (icon::WRENCH, "Various bug fixes"),
        ],
    },
    Release {
        version: "0.2.7",
        title: "AniList and new languages",
        changes: &[
            (icon::ARROWS_CLOCKWISE, "AniList sync: connect your account, your manga list comes into the library, and reading progress, the library and favourites stay in sync"),
            (icon::TRANSLATE, "Yomikaze now speaks Hungarian, Spanish, Japanese and Chinese, and there's a pirate language too"),
            (icon::STAR, "Finished a series? It moves to Completed on AniList and you can rate it there"),
            (icon::BOOK_OPEN, "Two pages side by side in paged mode, by itself in a wide window or with the T key"),
            (icon::GEAR_SIX, "Settings are sorted into categories"),
            (icon::IMAGE, "Backgrounds: anime wallpapers by genre and series, the one you pick is saved on your PC so it works offline too"),
            (icon::LIST, "Reader: chapter list on the C key, adjustable page width, smooth scrolling with the arrow keys, and the next chapter is ready before you get there"),
            (icon::WRENCH, "Various bug fixes"),
        ],
    },
    Release {
        version: "0.2.6",
        title: "Stay in sync",
        changes: &[
            (icon::ARROWS_LEFT_RIGHT, "Backups: export your library and the page you're on in every chapter, and import it on your iPhone or another PC"),
            (icon::EYE_SLASH, "Distraction-free reading: the controls stay hidden until you click the small button in the corner or press Tab"),
            (icon::PACKAGE, "Installer: Yomikaze installs into its own folder with a Start menu shortcut (and a desktop one if you like); your library stays when you reinstall"),
            (icon::WRENCH, "Lots of fixes to the look: solid dialogs, clearer buttons and switches, and layouts that fit smaller laptop screens"),
        ],
    },
    Release {
    version: "0.2.5",
    title: "Yomikaze for Windows",
    changes: &[
        (icon::DESKTOP, "Yomikaze now runs on Windows, with the same features as on iPhone"),
        (icon::HAND_TAP, "Tap zones: click the left or right edge to turn the page; arrow keys and the mouse wheel work too"),
        (icon::GEAR_SIX, "Reader options: background colour, page dimming and a gap between pages, right inside the reader"),
        (icon::FOLDER, "Library categories: make your own and sort your series into them"),
        (icon::LOCK_KEY, "Lock the app with Windows Hello: face, fingerprint or PIN"),
        (icon::IMAGE, "Share a series as a picture card, copied straight to the clipboard"),
        (icon::CHART_BAR, "Statistics: last read, totals, streak, the last 7 days and progress for every series"),
        (icon::PAINT_BRUSH, "Custom accent colour and your own background image with position, zoom and dimming"),
        (icon::WIFI_SLASH, "Offline mode and downloads for reading without a connection"),
        (icon::SPARKLE, "Launch animation and this What's New screen"),
    ],
    },
];

pub fn changelog_list(ui: &mut Ui, palette: &Palette) {
    for (index, release) in RELEASES.iter().enumerate() {
        theme::glass_frame(palette).fill(palette.glass_strong.gamma_multiply(0.75)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(RichText::new(release.version).font(theme::heavy_font(24.0)));
                ui.label(RichText::new(t(release.title)).color(palette.secondary));
                if index == 0 {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let galley = ui.painter().layout_no_wrap(t("LATEST").into(), theme::bold_font(11.0), theme::readable_on(palette.accent));
                        let (rect, _) = ui.allocate_exact_size(galley.size() + vec2(18.0, 8.0), Sense::hover());
                        ui.painter().rect_filled(rect, CornerRadius::same(255), palette.accent);
                        ui.painter().galley(rect.min + vec2(9.0, 4.0), galley, Color32::WHITE);
                    });
                }
            });
            ui.add_space(8.0);
            for (glyph, text) in release.changes {
                ui.horizontal_top(|ui| {
                    ui.label(RichText::new(*glyph).size(17.0).color(palette.accent));
                    ui.add(egui::Label::new(RichText::new(t(text)).size(14.5)).wrap());
                });
                ui.add_space(3.0);
            }
        });
        ui.add_space(14.0);
    }
}

pub fn changelog_page(ui: &mut Ui, shared: &mut Shared) {
    let palette = Palette::of(ui.ctx());
    if ui.input(|i| i.key_pressed(egui::Key::Escape) || i.pointer.button_clicked(egui::PointerButton::Extra1)) {
        shared.router.pop();
        return;
    }
    page(ui, "changelog", |ui| {
        if widgets::glass_button(ui, t("Back"), Some(icon::CARET_LEFT), None).clicked() {
            shared.router.pop();
        }
        widgets::page_title(ui, t("What's New"));
        ui.set_max_width(760.0);
        changelog_list(ui, &palette);
    });
}
