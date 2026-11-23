// everything saved on the pc, in %APPDATA%\Yomikaze

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::time::Instant;

use serde::{Deserialize, Serialize};
use serde::de::DeserializeOwned;

use crate::model::{Chapter, ChapterProgress, DownloadedChapter, SavedManga, now};
use crate::i18n::t;

// tests set YOMIKAZE_DATA_DIR so they never touch the real library
fn override_dir() -> Option<PathBuf> {
    std::env::var_os("YOMIKAZE_DATA_DIR").map(PathBuf::from)
}

pub fn data_dir() -> PathBuf {
    let base = override_dir().map(|d| d.join("data")).or_else(dirs::data_dir).unwrap_or_else(|| PathBuf::from("."));
    let dir = base.join("Yomikaze");
    let _ = fs::create_dir_all(&dir);
    dir
}

pub fn wallpaper_path() -> PathBuf {
    data_dir().join("wallpaper.png")
}

// changes whenever a new wallpaper is picked
pub fn wallpaper_url(rev: u32) -> String {
    format!("file:{}?{rev}", wallpaper_path().display())
}

pub fn cache_dir() -> PathBuf {
    let base = override_dir().map(|d| d.join("cache")).or_else(dirs::cache_dir).unwrap_or_else(|| PathBuf::from("."));
    let dir = base.join("Yomikaze");
    let _ = fs::create_dir_all(&dir);
    dir
}

fn read_json<T: DeserializeOwned>(path: &PathBuf) -> Option<T> {
    let bytes = fs::read(path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn write_json<T: Serialize>(path: &PathBuf, value: &T) {
    if let Ok(bytes) = serde_json::to_vec(value) {
        let tmp = path.with_extension("tmp");
        if fs::write(&tmp, bytes).is_ok() {
            let _ = fs::rename(&tmp, path);
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Appearance {
    System,
    Light,
    Dark,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReaderMode {
    Vertical,
    PagedLtr,
    PagedRtl,
}

impl ReaderMode {
    pub const ALL: [ReaderMode; 3] = [ReaderMode::Vertical, ReaderMode::PagedLtr, ReaderMode::PagedRtl];

    pub fn title(self) -> &'static str {
        match self {
            ReaderMode::Vertical => t("Vertical scroll"),
            ReaderMode::PagedLtr => t("Paged, left to right"),
            ReaderMode::PagedRtl => t("Paged, right to left"),
        }
    }
}

// two pages side by side in paged mode: by itself when the window is wide, or always or never
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TwoPages {
    #[default]
    Auto,
    On,
    Off,
}

impl TwoPages {
    pub const ALL: [TwoPages; 3] = [TwoPages::Auto, TwoPages::On, TwoPages::Off];

    pub fn title(self) -> &'static str {
        match self {
            TwoPages::Auto => t("Auto"),
            TwoPages::On => t("On"),
            TwoPages::Off => t("Off"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReaderBackground {
    Black,
    Gray,
    Sepia,
    White,
}

impl ReaderBackground {
    pub const ALL: [ReaderBackground; 4] =
        [ReaderBackground::Black, ReaderBackground::Gray, ReaderBackground::Sepia, ReaderBackground::White];

    pub fn name(self) -> &'static str {
        match self {
            ReaderBackground::Black => t("Black"),
            ReaderBackground::Gray => t("Grey"),
            ReaderBackground::Sepia => t("Sepia"),
            ReaderBackground::White => t("White"),
        }
    }

    pub fn color(self) -> egui::Color32 {
        match self {
            ReaderBackground::Black => egui::Color32::BLACK,
            ReaderBackground::Gray => egui::Color32::from_gray(36),
            ReaderBackground::Sepia => egui::Color32::from_rgb(237, 222, 194),
            ReaderBackground::White => egui::Color32::WHITE,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub accent: String,
    pub custom_accent: [u8; 3],
    pub appearance: Appearance,
    pub reader_mode: ReaderMode,
    pub keep_awake: bool,
    pub show_adult: bool,
    pub library_sort: String,
    pub recent_searches: Vec<String>,
    pub offline_mode: bool,
    pub wallpaper_dim: f32,
    pub wallpaper_zoom: f32,
    pub wallpaper_x: f32,
    pub wallpaper_y: f32,
    pub tap_zones: bool,
    pub reader_background: ReaderBackground,
    pub reader_dim: f32,
    pub reader_gap: bool,
    pub two_pages: TwoPages,
    // widest a page gets in vertical mode, ctrl + wheel changes it in the reader
    pub reader_width: f32,
    pub page_animation: bool,
    pub reader_progress_bar: bool,
    pub app_lock: bool,
    pub categories: Vec<String>,
    pub last_seen_version: String,
    // "system" or a language code from i18n::LANGUAGES
    pub language: String,
    pub anilist_user: Option<crate::anilist::User>,
    pub anilist_last_sync: i64,
    // anilist entries that weren't found on weebcentral, not looked up again on automatic syncs
    pub anilist_skip: Vec<i64>,
    pub ui_scale: f32,
    // 0 means no wallpaper
    pub wallpaper_rev: u32,
    // link of the premade picture in use, empty for one picked from the pc
    pub wallpaper_source: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            accent: "yoru".to_string(),
            custom_accent: [125, 115, 255],
            appearance: Appearance::Dark,
            reader_mode: ReaderMode::Vertical,
            keep_awake: true,
            show_adult: false,
            library_sort: "recentlyAdded".to_string(),
            recent_searches: Vec::new(),
            offline_mode: false,
            wallpaper_dim: 0.6,
            wallpaper_zoom: 1.0,
            wallpaper_x: 0.0,
            wallpaper_y: 0.0,
            tap_zones: true,
            reader_background: ReaderBackground::Black,
            reader_dim: 0.0,
            reader_gap: false,
            two_pages: TwoPages::Auto,
            reader_width: 980.0,
            page_animation: true,
            reader_progress_bar: true,
            app_lock: false,
            categories: Vec::new(),
            last_seen_version: String::new(),
            language: "system".to_string(),
            anilist_user: None,
            anilist_last_sync: 0,
            anilist_skip: Vec::new(),
            ui_scale: 1.0,
            wallpaper_rev: 0,
            wallpaper_source: String::new(),
        }
    }
}

#[derive(Default, Serialize, Deserialize)]
struct Snapshot {
    manga: Vec<SavedManga>,
    progress: Vec<ChapterProgress>,
    downloads: Vec<DownloadedChapter>,
}

pub struct Store {
    pub manga: HashMap<String, SavedManga>,
    pub progress: HashMap<String, ChapterProgress>,
    pub downloads: HashMap<String, DownloadedChapter>,
    pub settings: Settings,
    dirty_data: bool,
    dirty_settings: bool,
    last_save: Instant,
    // series whose library state or reading progress changed, the anilist sync pushes them
    pub sync_changed: HashSet<String>,
}

impl Store {
    pub fn load() -> Self {
        let dir = data_dir();
        let snapshot: Snapshot = read_json(&dir.join("library.json")).unwrap_or_default();
        let settings: Settings = read_json(&dir.join("settings.json")).unwrap_or_default();
        crate::net::set_offline_forced(settings.offline_mode);
        Self {
            manga: snapshot.manga.into_iter().map(|m| (m.manga_id.clone(), m)).collect(),
            progress: snapshot.progress.into_iter().map(|p| (p.chapter_id.clone(), p)).collect(),
            downloads: snapshot.downloads.into_iter().map(|d| (d.chapter_id.clone(), d)).collect(),
            settings,
            dirty_data: false,
            dirty_settings: false,
            last_save: Instant::now(),
            sync_changed: HashSet::new(),
        }
    }

    #[cfg(test)]
    pub fn empty() -> Self {
        Self {
            manga: HashMap::new(),
            progress: HashMap::new(),
            downloads: HashMap::new(),
            settings: Settings::default(),
            dirty_data: false,
            dirty_settings: false,
            last_save: Instant::now(),
            sync_changed: HashSet::new(),
        }
    }

    pub fn mark_dirty(&mut self) {
        self.dirty_data = true;
    }

    pub fn settings_changed(&mut self) {
        self.dirty_settings = true;
        crate::net::set_offline_forced(self.settings.offline_mode);
    }

    // writes at most every couple of seconds, force on exit
    pub fn flush(&mut self, force: bool) {
        if !force && self.last_save.elapsed().as_secs_f32() < 2.0 {
            return;
        }
        let dir = data_dir();
        if self.dirty_data {
            let snapshot = Snapshot {
                manga: self.manga.values().cloned().collect(),
                progress: self.progress.values().cloned().collect(),
                downloads: self.downloads.values().cloned().collect(),
            };
            write_json(&dir.join("library.json"), &snapshot);
            self.dirty_data = false;
        }
        if self.dirty_settings {
            write_json(&dir.join("settings.json"), &self.settings);
            self.dirty_settings = false;
        }
        self.last_save = Instant::now();
    }

    pub fn saved(&self, id: &str) -> Option<&SavedManga> {
        self.manga.get(id)
    }

    pub fn upsert(&mut self, id: &str, title: &str) -> &mut SavedManga {
        self.dirty_data = true;
        self.manga.entry(id.to_string()).or_insert_with(|| SavedManga::new(id, title))
    }

    pub fn in_library(&self, id: &str) -> bool {
        self.manga.get(id).map(|m| m.in_library).unwrap_or(false)
    }

    pub fn toggle_library(&mut self, id: &str, title: &str, author: &str, status: &str, kind: &str) -> bool {
        let entry = self.upsert(id, title);
        entry.in_library = !entry.in_library;
        let now_in = entry.in_library;
        if now_in {
            entry.date_added = now();
            entry.title = title.to_string();
            if !author.is_empty() {
                entry.author = author.to_string();
            }
            if !status.is_empty() {
                entry.status = status.to_string();
            }
            if !kind.is_empty() {
                entry.kind = kind.to_string();
            }
        } else if !entry.has_history {
            self.manga.remove(id);
        }
        self.dirty_data = true;
        self.sync_changed.insert(id.to_string());
        now_in
    }

    pub fn remove_from_library(&mut self, id: &str) {
        if let Some(entry) = self.manga.get_mut(id) {
            entry.in_library = false;
            if !entry.has_history {
                self.manga.remove(id);
            }
        }
        self.dirty_data = true;
    }

    // also moves the manga to the top of the history
    pub fn record_progress(&mut self, manga_id: &str, manga_title: &str, chapter: &Chapter, page: usize, page_count: usize, completed: bool) {
        let entry = self.upsert(manga_id, manga_title);
        entry.has_history = true;
        entry.last_read_at = now();
        entry.last_chapter_id = chapter.id.clone();
        entry.last_chapter_name = chapter.name.clone();
        entry.last_page = page;
        entry.last_page_count = page_count;

        let started = !self.progress.contains_key(&chapter.id);
        let item = self.progress.entry(chapter.id.clone()).or_insert_with(|| ChapterProgress {
            chapter_id: chapter.id.clone(),
            manga_id: manga_id.to_string(),
            chapter_name: chapter.name.clone(),
            page,
            page_count,
            completed,
            updated_at: now(),
        });
        let finished = completed && !item.completed;
        item.page = page;
        item.page_count = page_count;
        item.completed = item.completed || completed;
        item.updated_at = now();
        self.dirty_data = true;
        // a new chapter or a finished one is worth telling anilist about
        if started || finished {
            self.sync_changed.insert(manga_id.to_string());
        }
    }

    pub fn set_read(&mut self, manga_id: &str, chapter: &Chapter, read: bool) {
        if read {
            let item = self.progress.entry(chapter.id.clone()).or_insert_with(|| ChapterProgress {
                chapter_id: chapter.id.clone(),
                manga_id: manga_id.to_string(),
                chapter_name: chapter.name.clone(),
                page: 0,
                page_count: 0,
                completed: true,
                updated_at: now(),
            });
            item.completed = true;
        } else {
            self.progress.remove(&chapter.id);
        }
        self.dirty_data = true;
        self.sync_changed.insert(manga_id.to_string());
    }

    // what the anilist sync needs to know about each series, or just the ones in `only`
    pub fn anilist_locals(&self, only: Option<&HashSet<String>>) -> Vec<crate::anilist::Local> {
        let mut read: HashMap<&str, i64> = HashMap::new();
        for item in self.progress.values() {
            let Some(number) = crate::anilist::chapter_number(&item.chapter_name) else { continue };
            // a started chapter means the ones before it are read
            let done = if item.completed { number } else { number - 1 };
            let best = read.entry(item.manga_id.as_str()).or_insert(0);
            *best = (*best).max(done);
        }
        self.manga
            .values()
            .filter(|m| only.is_none_or(|ids| ids.contains(&m.manga_id)))
            .map(|m| crate::anilist::Local {
                manga_id: m.manga_id.clone(),
                title: m.title.clone(),
                in_library: m.in_library,
                anilist_id: m.anilist_id,
                progress: read.get(m.manga_id.as_str()).copied().unwrap_or(0).max(0),
                started: m.has_history || read.contains_key(m.manga_id.as_str()),
            })
            .collect()
    }

    // applies a sync result. returns the series that were just finished, for the rating prompt.
    pub fn apply_anilist(&mut self, outcome: crate::anilist::Outcome) -> Vec<(String, i64)> {
        for (manga_id, id) in outcome.links {
            if let Some(m) = self.manga.get_mut(&manga_id) {
                m.anilist_id = Some(id);
            }
        }
        for item in outcome.imported {
            let detail = item.detail;
            let entry = self.upsert(&detail.id, &detail.title);
            if !entry.in_library {
                entry.in_library = true;
                entry.date_added = now();
            }
            entry.author = detail.authors.join(", ");
            entry.status = detail.status.clone();
            entry.kind = detail.kind.clone();
            entry.anilist_id = Some(item.anilist_id);
        }
        for reads in outcome.reads {
            self.apply_remote_reads(reads);
        }
        if outcome.full {
            for id in outcome.unmatched {
                if !self.settings.anilist_skip.contains(&id) {
                    self.settings.anilist_skip.push(id);
                }
            }
            self.settings.anilist_last_sync = now();
        }
        self.dirty_data = true;
        self.settings_changed();
        outcome.finished
    }

    // anilist is further than the app: mark those chapters read and move the history there
    fn apply_remote_reads(&mut self, reads: crate::anilist::RemoteReads) {
        let Some(entry) = self.manga.get_mut(&reads.manga_id) else { return };
        if reads.chapters.is_empty() {
            // no chapter list yet, applied when the series page loads it
            entry.anilist_progress = Some(reads.progress);
            return;
        }
        let last = reads
            .chapters
            .iter()
            .max_by_key(|c| crate::anilist::chapter_number(&c.name).unwrap_or(0))
            .cloned();
        if let Some(last) = last {
            entry.has_history = true;
            entry.last_chapter_id = last.id.clone();
            entry.last_chapter_name = last.name.clone();
            entry.last_page = 0;
            entry.last_page_count = 0;
            if reads.updated_at > entry.last_read_at {
                entry.last_read_at = reads.updated_at;
            }
        }
        for chapter in &reads.chapters {
            let item = self.progress.entry(chapter.id.clone()).or_insert_with(|| ChapterProgress {
                chapter_id: chapter.id.clone(),
                manga_id: reads.manga_id.clone(),
                chapter_name: chapter.name.clone(),
                page: 0,
                page_count: 0,
                completed: true,
                updated_at: reads.updated_at,
            });
            item.completed = true;
        }
    }

    // it stays stored while it is still in the library
    pub fn remove_from_history(&mut self, id: &str) {
        self.progress.retain(|_, p| p.manga_id != id);
        if let Some(entry) = self.manga.get_mut(id) {
            if entry.in_library {
                entry.has_history = false;
                entry.last_read_at = 0;
                entry.last_chapter_id.clear();
                entry.last_chapter_name.clear();
                entry.last_page = 0;
                entry.last_page_count = 0;
            } else {
                self.manga.remove(id);
            }
        }
        self.dirty_data = true;
    }

    pub fn history(&self) -> Vec<&SavedManga> {
        let mut list: Vec<&SavedManga> = self.manga.values().filter(|m| m.has_history).collect();
        list.sort_by(|a, b| b.last_read_at.cmp(&a.last_read_at));
        list
    }

    pub fn library(&self) -> Vec<&SavedManga> {
        let mut list: Vec<&SavedManga> = self.manga.values().filter(|m| m.in_library).collect();
        match self.settings.library_sort.as_str() {
            "lastRead" => list.sort_by(|a, b| b.last_read_at.cmp(&a.last_read_at)),
            "title" => list.sort_by_key(|m| m.title.to_lowercase()),
            _ => list.sort_by(|a, b| b.date_added.cmp(&a.date_added)),
        }
        list
    }

    pub fn remember_search(&mut self, term: &str) {
        let term = term.trim();
        if term.is_empty() {
            return;
        }
        let recent = &mut self.settings.recent_searches;
        recent.retain(|t| !t.eq_ignore_ascii_case(term));
        recent.insert(0, term.to_string());
        recent.truncate(8);
        self.settings_changed();
    }
}

// catalog cache: series details, chapter lists and artwork, for instant and offline opening

pub mod catalog {
    use super::*;

    fn dir() -> PathBuf {
        let dir = data_dir().join("Catalog");
        let _ = fs::create_dir_all(&dir);
        dir
    }

    pub fn load<T: DeserializeOwned>(name: &str) -> Option<T> {
        read_json(&dir().join(format!("{name}.json")))
    }

    pub fn save<T: Serialize>(name: &str, value: &T) {
        write_json(&dir().join(format!("{name}.json")), value);
    }

    pub fn detail_name(id: &str) -> String {
        format!("detail-{id}")
    }

    pub fn chapters_name(id: &str) -> String {
        format!("chapters-{id}")
    }

    pub fn artwork_name(id: &str) -> String {
        format!("artwork-{id}")
    }

    pub fn chapters(id: &str) -> Vec<Chapter> {
        load(&chapters_name(id)).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chapter(id: &str, name: &str) -> Chapter {
        Chapter { id: id.to_string(), name: name.to_string(), date: None }
    }

    #[test]
    fn anilist_progress_counts_started_chapters() {
        let mut store = Store::empty();
        // started chapter 5 without finishing: chapters 1 to 4 count as read
        store.record_progress("M1", "One", &chapter("c5", "Chapter 5"), 3, 20, false);
        let local = store.anilist_locals(None);
        assert_eq!(local[0].progress, 4);
        assert!(local[0].started);
        assert!(store.sync_changed.contains("M1"));

        // finishing chapter 7 moves it to 7
        store.record_progress("M1", "One", &chapter("c7", "Chapter 7"), 19, 20, true);
        assert_eq!(store.anilist_locals(None)[0].progress, 7);

        // only the asked series come back
        store.toggle_library("M2", "Two", "", "", "");
        let only: HashSet<String> = ["M2".to_string()].into_iter().collect();
        let picked = store.anilist_locals(Some(&only));
        assert_eq!(picked.len(), 1);
        assert_eq!(picked[0].manga_id, "M2");
        assert_eq!(picked[0].progress, 0);
        assert!(!picked[0].started);
    }
}
