use serde::{Deserialize, Serialize};
use crate::i18n::{t, tf};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MangaSummary {
    pub id: String,
    pub title: String,
    pub status: Option<String>,
    pub kind: Option<String>,
    pub year: Option<String>,
    pub latest_chapter: Option<String>,
    pub latest_chapter_id: Option<String>,
    // unix seconds
    pub updated_at: Option<i64>,
}

impl MangaSummary {
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self { id: id.into(), title: title.into(), ..Default::default() }
    }

    pub fn meta_line(&self) -> String {
        [&self.kind, &self.status, &self.year]
            .into_iter()
            .flatten()
            .filter(|s| !s.is_empty())
            .cloned()
            .collect::<Vec<_>>()
            .join(" · ")
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MangaDetail {
    pub id: String,
    pub title: String,
    pub authors: Vec<String>,
    pub tags: Vec<String>,
    pub kind: String,
    pub status: String,
    pub released: String,
    pub official: bool,
    pub adult: bool,
    pub summary: String,
    pub anilist_id: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Chapter {
    pub id: String,
    pub name: String,
    // unix seconds
    pub date: Option<i64>,
}

#[derive(Clone, Debug, Default)]
pub struct ResultPage {
    pub items: Vec<MangaSummary>,
    pub has_more: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Artwork {
    pub banner: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SavedManga {
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
    #[serde(default)]
    pub last_read_at: i64,
    #[serde(default)]
    pub last_chapter_id: String,
    #[serde(default)]
    pub last_chapter_name: String,
    #[serde(default)]
    pub last_page: usize,
    #[serde(default)]
    pub last_page_count: usize,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub anilist_id: Option<i64>,
    // chapters read according to anilist, applied when the chapter list is loaded
    #[serde(default)]
    pub anilist_progress: Option<i64>,
}

impl SavedManga {
    pub fn new(id: &str, title: &str) -> Self {
        Self {
            manga_id: id.to_string(),
            title: title.to_string(),
            author: String::new(),
            status: String::new(),
            kind: String::new(),
            in_library: false,
            date_added: now(),
            has_history: false,
            last_read_at: 0,
            last_chapter_id: String::new(),
            last_chapter_name: String::new(),
            last_page: 0,
            last_page_count: 0,
            category: String::new(),
            anilist_id: None,
            anilist_progress: None,
        }
    }

    pub fn summary(&self) -> MangaSummary {
        MangaSummary {
            id: self.manga_id.clone(),
            title: self.title.clone(),
            status: (!self.status.is_empty()).then(|| self.status.clone()),
            kind: (!self.kind.is_empty()).then(|| self.kind.clone()),
            ..Default::default()
        }
    }

    pub fn progress_line(&self) -> String {
        if self.last_page_count > 0 {
            format!("{} · {}/{}", self.last_chapter_name, self.last_page + 1, self.last_page_count)
        } else {
            self.last_chapter_name.clone()
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChapterProgress {
    pub chapter_id: String,
    pub manga_id: String,
    pub chapter_name: String,
    pub page: usize,
    pub page_count: usize,
    pub completed: bool,
    pub updated_at: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DownloadedChapter {
    pub chapter_id: String,
    pub manga_id: String,
    pub manga_title: String,
    pub chapter_name: String,
    pub page_urls: Vec<String>,
    pub downloaded_at: i64,
}

pub fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

pub fn relative_time(timestamp: i64) -> String {
    let seconds = (now() - timestamp).max(0);
    let minutes = seconds / 60;
    let hours = minutes / 60;
    let days = hours / 24;
    match () {
        _ if seconds < 60 => t("just now").to_string(),
        _ if minutes < 60 => plural(minutes, "1 minute ago", "{n} minutes ago"),
        _ if hours < 24 => plural(hours, "1 hour ago", "{n} hours ago"),
        _ if days == 1 => t("yesterday").to_string(),
        _ if days < 30 => plural(days, "1 day ago", "{n} days ago"),
        _ if days < 365 => plural(days / 30, "1 month ago", "{n} months ago"),
        _ => plural(days / 365, "1 year ago", "{n} years ago"),
    }
}

fn plural(value: i64, one: &str, many: &str) -> String {
    if value == 1 { t(one).to_string() } else { tf(many, &[("n", &value)]) }
}

pub fn short_date(timestamp: i64) -> String {
    chrono::DateTime::from_timestamp(timestamp, 0)
        .map(|d| d.with_timezone(&chrono::Local).format("%b %-d, %Y").to_string())
        .unwrap_or_default()
}
