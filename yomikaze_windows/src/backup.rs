// backup file shared with the ios app ("yomikaze-backup", version 1)
// importing merges: nothing is deleted and the more recent position wins

use serde::{Deserialize, Serialize};

use crate::model::{ChapterProgress, SavedManga, now};
use crate::store::{Appearance, ReaderBackground, ReaderMode, Store};
use crate::theme::CUSTOM_ACCENT;

pub const FORMAT: &str = "yomikaze-backup";
pub const VERSION: u32 = 1;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupFile {
    pub format: String,
    pub version: u32,
    pub app: String,
    pub app_version: String,
    // unix seconds
    pub exported_at: i64,
    pub manga: Vec<BackupManga>,
    pub progress: Vec<BackupProgress>,
    #[serde(default)]
    pub categories: Vec<String>,
    #[serde(default)]
    pub recent_searches: Vec<String>,
    #[serde(default)]
    pub settings: Option<BackupSettings>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupManga {
    pub manga_id: String,
    pub title: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub in_library: bool,
    #[serde(default)]
    pub date_added: i64,
    #[serde(default)]
    pub has_history: bool,
    // unix seconds, 0 when never read
    #[serde(default)]
    pub last_read_at: i64,
    #[serde(default)]
    pub last_chapter_id: String,
    #[serde(default)]
    pub last_chapter_name: String,
    // zero-based
    #[serde(default)]
    pub last_page: usize,
    #[serde(default)]
    pub last_page_count: usize,
    #[serde(default)]
    pub category: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupProgress {
    pub chapter_id: String,
    pub manga_id: String,
    #[serde(default)]
    pub chapter_name: String,
    // zero-based
    #[serde(default)]
    pub page: usize,
    #[serde(default)]
    pub page_count: usize,
    #[serde(default)]
    pub completed: bool,
    #[serde(default)]
    pub updated_at: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupSettings {
    pub accent: String,
    // "#RRGGBB"
    pub custom_accent: String,
    // system, light or dark
    pub appearance: String,
    // vertical, pagedLTR or pagedRTL
    pub reader_mode: String,
    pub tap_zones: bool,
    // black, gray, sepia or white
    pub reader_background: String,
    pub reader_dim: f64,
    pub reader_gap: bool,
    pub keep_awake: bool,
    pub show_adult: bool,
    // recentlyAdded, lastRead or title
    pub library_sort: String,
}

#[derive(Default, Debug)]
pub struct Summary {
    pub series: usize,
    pub chapters: usize,
}

pub fn export(store: &Store) -> BackupFile {
    let s = &store.settings;
    let [r, g, b] = s.custom_accent;
    BackupFile {
        format: FORMAT.to_string(),
        version: VERSION,
        app: "windows".to_string(),
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        exported_at: now(),
        manga: store
            .manga
            .values()
            .map(|m| BackupManga {
                manga_id: m.manga_id.clone(),
                title: m.title.clone(),
                author: m.author.clone(),
                status: m.status.clone(),
                kind: m.kind.clone(),
                in_library: m.in_library,
                date_added: m.date_added,
                has_history: m.has_history,
                last_read_at: m.last_read_at,
                last_chapter_id: m.last_chapter_id.clone(),
                last_chapter_name: m.last_chapter_name.clone(),
                last_page: m.last_page,
                last_page_count: m.last_page_count,
                category: m.category.clone(),
            })
            .collect(),
        progress: store
            .progress
            .values()
            .map(|p| BackupProgress {
                chapter_id: p.chapter_id.clone(),
                manga_id: p.manga_id.clone(),
                chapter_name: p.chapter_name.clone(),
                page: p.page,
                page_count: p.page_count,
                completed: p.completed,
                updated_at: p.updated_at,
            })
            .collect(),
        categories: s.categories.clone(),
        recent_searches: s.recent_searches.clone(),
        settings: Some(BackupSettings {
            accent: s.accent.clone(),
            custom_accent: format!("#{r:02X}{g:02X}{b:02X}"),
            appearance: match s.appearance {
                Appearance::System => "system",
                Appearance::Light => "light",
                Appearance::Dark => "dark",
            }
            .to_string(),
            reader_mode: match s.reader_mode {
                ReaderMode::Vertical => "vertical",
                ReaderMode::PagedLtr => "pagedLTR",
                ReaderMode::PagedRtl => "pagedRTL",
            }
            .to_string(),
            tap_zones: s.tap_zones,
            reader_background: match s.reader_background {
                ReaderBackground::Black => "black",
                ReaderBackground::Gray => "gray",
                ReaderBackground::Sepia => "sepia",
                ReaderBackground::White => "white",
            }
            .to_string(),
            reader_dim: s.reader_dim as f64,
            reader_gap: s.reader_gap,
            keep_awake: s.keep_awake,
            show_adult: s.show_adult,
            library_sort: s.library_sort.clone(),
        }),
    }
}

pub fn file_name() -> String {
    format!("Yomikaze-backup-{}.json", chrono::Local::now().format("%Y-%m-%d"))
}

pub fn parse(bytes: &[u8]) -> Result<BackupFile, String> {
    let file: BackupFile = serde_json::from_slice(bytes).map_err(|e| format!("This isn't a Yomikaze backup ({e})."))?;
    if file.format != FORMAT {
        return Err("This isn't a Yomikaze backup.".to_string());
    }
    if file.version > VERSION {
        return Err("This backup was made by a newer Yomikaze. Update the app first.".to_string());
    }
    Ok(file)
}

// with_settings also takes over the reader and appearance options
pub fn apply(store: &mut Store, file: &BackupFile, with_settings: bool) -> Summary {
    let mut summary = Summary::default();

    for incoming in &file.manga {
        if incoming.manga_id.is_empty() {
            continue;
        }
        summary.series += 1;
        let entry = store
            .manga
            .entry(incoming.manga_id.clone())
            .or_insert_with(|| SavedManga::new(&incoming.manga_id, &incoming.title));

        if entry.title.is_empty() {
            entry.title = incoming.title.clone();
        }
        for (field, value) in [
            (&mut entry.author, &incoming.author),
            (&mut entry.status, &incoming.status),
            (&mut entry.kind, &incoming.kind),
        ] {
            if field.is_empty() && !value.is_empty() {
                *field = value.clone();
            }
        }
        if incoming.in_library {
            if !entry.in_library {
                entry.date_added = incoming.date_added;
            }
            entry.in_library = true;
        }
        if entry.category.is_empty() && !incoming.category.is_empty() {
            entry.category = incoming.category.clone();
        }
        if incoming.has_history && (!entry.has_history || incoming.last_read_at > entry.last_read_at) {
            entry.has_history = true;
            entry.last_read_at = incoming.last_read_at;
            entry.last_chapter_id = incoming.last_chapter_id.clone();
            entry.last_chapter_name = incoming.last_chapter_name.clone();
            entry.last_page = incoming.last_page;
            entry.last_page_count = incoming.last_page_count;
        }
    }

    for incoming in &file.progress {
        if incoming.chapter_id.is_empty() {
            continue;
        }
        summary.chapters += 1;
        match store.progress.get_mut(&incoming.chapter_id) {
            Some(existing) => {
                let completed = existing.completed || incoming.completed;
                if incoming.updated_at > existing.updated_at {
                    existing.page = incoming.page;
                    existing.page_count = incoming.page_count;
                    existing.updated_at = incoming.updated_at;
                    if !incoming.chapter_name.is_empty() {
                        existing.chapter_name = incoming.chapter_name.clone();
                    }
                }
                existing.completed = completed;
            }
            None => {
                store.progress.insert(
                    incoming.chapter_id.clone(),
                    ChapterProgress {
                        chapter_id: incoming.chapter_id.clone(),
                        manga_id: incoming.manga_id.clone(),
                        chapter_name: incoming.chapter_name.clone(),
                        page: incoming.page,
                        page_count: incoming.page_count,
                        completed: incoming.completed,
                        updated_at: incoming.updated_at,
                    },
                );
            }
        }
    }

    let settings = &mut store.settings;
    for name in &file.categories {
        if !name.trim().is_empty() && !settings.categories.iter().any(|c| c.eq_ignore_ascii_case(name)) {
            settings.categories.push(name.clone());
        }
    }
    for name in store.manga.values().map(|m| m.category.clone()).filter(|c| !c.is_empty()).collect::<Vec<_>>() {
        if !settings.categories.iter().any(|c| c.eq_ignore_ascii_case(&name)) {
            settings.categories.push(name);
        }
    }
    for term in &file.recent_searches {
        if !settings.recent_searches.iter().any(|t| t.eq_ignore_ascii_case(term)) {
            settings.recent_searches.push(term.clone());
        }
    }
    settings.recent_searches.truncate(8);

    if with_settings {
        if let Some(incoming) = &file.settings {
            settings.accent = incoming.accent.clone();
            if let Some(rgb) = parse_hex(&incoming.custom_accent) {
                settings.custom_accent = rgb;
            } else if settings.accent == CUSTOM_ACCENT {
                settings.accent = "yoru".to_string();
            }
            settings.appearance = match incoming.appearance.as_str() {
                "light" => Appearance::Light,
                "dark" => Appearance::Dark,
                _ => Appearance::System,
            };
            settings.reader_mode = match incoming.reader_mode.as_str() {
                "pagedLTR" => ReaderMode::PagedLtr,
                "pagedRTL" => ReaderMode::PagedRtl,
                _ => ReaderMode::Vertical,
            };
            settings.reader_background = match incoming.reader_background.as_str() {
                "gray" => ReaderBackground::Gray,
                "sepia" => ReaderBackground::Sepia,
                "white" => ReaderBackground::White,
                _ => ReaderBackground::Black,
            };
            settings.tap_zones = incoming.tap_zones;
            settings.reader_dim = (incoming.reader_dim as f32).clamp(0.0, 0.7);
            settings.reader_gap = incoming.reader_gap;
            settings.keep_awake = incoming.keep_awake;
            settings.show_adult = incoming.show_adult;
            if ["recentlyAdded", "lastRead", "title"].contains(&incoming.library_sort.as_str()) {
                settings.library_sort = incoming.library_sort.clone();
            }
        }
    }

    store.mark_dirty();
    store.settings_changed();
    summary
}

fn parse_hex(hex: &str) -> Option<[u8; 3]> {
    let clean = hex.trim().trim_start_matches('#');
    if clean.len() != 6 {
        return None;
    }
    let value = u32::from_str_radix(clean, 16).ok()?;
    Some([(value >> 16) as u8, (value >> 8) as u8, value as u8])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Chapter;

    fn empty_store() -> Store {
        Store::empty()
    }

    #[test]
    fn round_trip_keeps_pages_and_library() {
        let mut source = empty_store();
        let chapter = Chapter { id: "CH1".into(), name: "Chapter 1".into(), date: None };
        source.record_progress("M1", "One Piece", &chapter, 13, 57, false);
        source.toggle_library("M1", "One Piece", "ODA", "Ongoing", "Manga");
        source.manga.get_mut("M1").unwrap().category = "Reading".into();
        source.settings.categories = vec!["Reading".into()];
        source.settings.reader_mode = ReaderMode::PagedRtl;

        let json = serde_json::to_string_pretty(&export(&source)).unwrap();
        std::fs::write(std::env::temp_dir().join("yomikaze-backup-sample.json"), &json).unwrap();
        let parsed = parse(json.as_bytes()).unwrap();

        let mut target = empty_store();
        let summary = apply(&mut target, &parsed, true);
        assert_eq!(summary.series, 1);
        assert_eq!(summary.chapters, 1);
        let manga = &target.manga["M1"];
        assert!(manga.in_library && manga.has_history);
        assert_eq!((manga.last_page, manga.last_page_count), (13, 57));
        assert_eq!(manga.category, "Reading");
        assert_eq!(target.progress["CH1"].page, 13);
        assert_eq!(target.settings.reader_mode, ReaderMode::PagedRtl);
        assert_eq!(target.settings.categories, vec!["Reading".to_string()]);
    }

    #[test]
    fn newer_position_wins() {
        let chapter = Chapter { id: "CH1".into(), name: "Chapter 1".into(), date: None };
        let mut newer = empty_store();
        newer.record_progress("M1", "X", &chapter, 30, 40, false);
        let mut file = export(&newer);
        file.progress[0].updated_at += 100;
        file.manga[0].last_read_at += 100;

        let mut target = empty_store();
        target.record_progress("M1", "X", &chapter, 5, 40, false);
        apply(&mut target, &file, false);
        assert_eq!(target.progress["CH1"].page, 30);
        assert_eq!(target.manga["M1"].last_page, 30);
    }
}
