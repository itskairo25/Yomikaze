// client for weebcentral.com and anilist
// the site serves plain html fragments, so a few regex patterns are enough.
// if the markup changes, parser is the only place to update

use std::sync::OnceLock;
use std::time::Duration;

use regex::Regex;

use crate::model::{Artwork, Chapter, MangaDetail, MangaSummary, ResultPage};
use crate::net;

pub const BASE: &str = "https://weebcentral.com";
pub const PAGE_SIZE: usize = 32;
pub const USER_AGENT: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Mobile/15E148 Safari/604.1";

pub type Result<T> = std::result::Result<T, String>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SearchSort {
    BestMatch,
    Popularity,
    LatestUpdates,
    RecentlyAdded,
    Subscribers,
    Alphabet,
}

impl SearchSort {
    pub const ALL: [SearchSort; 6] = [
        SearchSort::BestMatch,
        SearchSort::Popularity,
        SearchSort::LatestUpdates,
        SearchSort::RecentlyAdded,
        SearchSort::Subscribers,
        SearchSort::Alphabet,
    ];

    pub fn label(self) -> &'static str {
        match self {
            SearchSort::BestMatch => "Best Match",
            SearchSort::Popularity => "Popularity",
            SearchSort::LatestUpdates => "Latest Updates",
            SearchSort::RecentlyAdded => "Recently Added",
            SearchSort::Subscribers => "Subscribers",
            SearchSort::Alphabet => "Alphabet",
        }
    }
}

pub fn cover_url(manga_id: &str, small: bool) -> String {
    format!(
        "https://temp.compsci88.com/cover/{}/{manga_id}.webp",
        if small { "small" } else { "normal" }
    )
}

pub fn series_url(manga_id: &str) -> String {
    format!("{BASE}/series/{manga_id}")
}

fn agent() -> &'static ureq::Agent {
    static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
    AGENT.get_or_init(|| {
        ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(30)))
            .user_agent(USER_AGENT)
            .http_status_as_error(false)
            .build()
            .into()
    })
}

fn friendly(error: ureq::Error) -> String {
    match error {
        ureq::Error::Timeout(_) => "WeebCentral took too long to answer.".to_string(),
        ureq::Error::HostNotFound | ureq::Error::ConnectionFailed => {
            "Couldn't reach WeebCentral. Check your connection.".to_string()
        }
        other => format!("Network error: {other}"),
    }
}

fn fetch_html(url: &str) -> Result<String> {
    if net::offline_forced() {
        return Err("Offline mode is on.".to_string());
    }
    let mut response = agent()
        .get(url)
        .header("Referer", &format!("{BASE}/"))
        .header("Accept-Language", "en-US,en;q=0.9")
        .call()
        .map_err(friendly)?;
    let status = response.status().as_u16();
    if !(200..300).contains(&status) {
        return Err(format!("WeebCentral answered with an error (HTTP {status})."));
    }
    response
        .body_mut()
        .with_config()
        .limit(64 * 1024 * 1024)
        .read_to_string()
        .map_err(|e| format!("Couldn't read the response: {e}"))
}

pub fn fetch_bytes(url: &str) -> Result<Vec<u8>> {
    if net::offline_forced() {
        return Err("Offline mode is on.".to_string());
    }
    let mut last_error = String::new();
    for attempt in 0..2 {
        let result = agent()
            .get(url)
            .header("Referer", &format!("{BASE}/"))
            .call()
            .map_err(friendly)
            .and_then(|mut response| {
                let status = response.status().as_u16();
                if !(200..300).contains(&status) {
                    return Err(format!("HTTP {status}"));
                }
                response
                    .body_mut()
                    .with_config()
                    .limit(64 * 1024 * 1024)
                    .read_to_vec()
                    .map_err(|e| e.to_string())
            });
        match result {
            Ok(bytes) if !bytes.is_empty() => return Ok(bytes),
            Ok(_) => last_error = "Empty image".to_string(),
            Err(error) => last_error = error,
        }
        if attempt == 0 {
            std::thread::sleep(Duration::from_millis(400));
        }
    }
    Err(last_error)
}

fn encode(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(byte as char),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

pub fn search(text: &str, sort: SearchSort, tag: Option<&str>, offset: usize, include_adult: bool) -> Result<ResultPage> {
    let order = if sort == SearchSort::Alphabet { "Ascending" } else { "Descending" };
    let mut url = format!(
        "{BASE}/search/data?text={}&sort={}&order={order}&official=Any&anime=Any&adult={}&display_mode={}&limit={PAGE_SIZE}&offset={offset}",
        encode(text),
        encode(sort.label()),
        if include_adult { "Any" } else { "False" },
        encode("Full Display"),
    );
    if let Some(tag) = tag.filter(|t| !t.is_empty()) {
        url.push_str(&format!("&included_tag={}", encode(tag)));
    }
    let html = fetch_html(&url)?;
    let has_more = html.contains(&format!("offset={}", offset + PAGE_SIZE));
    Ok(ResultPage { items: parser::cards(&html), has_more })
}

pub fn latest_updates(page: usize) -> Result<ResultPage> {
    let html = fetch_html(&format!("{BASE}/latest-updates/{page}"))?;
    let has_more = html.contains(&format!("latest-updates/{}", page + 1));
    Ok(ResultPage { items: parser::cards(&html), has_more })
}

pub fn hot_updates() -> Result<Vec<MangaSummary>> {
    Ok(parser::cards(&fetch_html(&format!("{BASE}/hot-updates"))?))
}

pub fn detail(manga_id: &str) -> Result<MangaDetail> {
    let html = fetch_html(&series_url(manga_id))?;
    parser::detail(&html, manga_id).ok_or_else(|| "Couldn't understand the series page.".to_string())
}

pub fn chapters(manga_id: &str) -> Result<Vec<Chapter>> {
    Ok(parser::chapters(&fetch_html(&format!("{BASE}/series/{manga_id}/full-chapter-list"))?))
}

pub fn pages(chapter_id: &str) -> Result<Vec<String>> {
    let html = fetch_html(&format!(
        "{BASE}/chapters/{chapter_id}/images?is_prev=False&current_page=1&reading_style=long_strip"
    ))?;
    let pages = parser::pages(&html);
    if pages.is_empty() {
        Err("This chapter has no pages.".to_string())
    } else {
        Ok(pages)
    }
}

// weebcentral links the exact anilist entry, the title search is only a fallback
pub fn artwork(anilist_id: Option<i64>, title: &str) -> Result<Artwork> {
    if net::offline_forced() {
        return Err("Offline mode is on.".to_string());
    }
    let body = match anilist_id {
        Some(id) => serde_json::json!({
            "query": "query ($id: Int) { Media(id: $id, type: MANGA) { bannerImage } }",
            "variables": { "id": id }
        }),
        None => serde_json::json!({
            "query": "query ($search: String) { Media(search: $search, type: MANGA) { bannerImage } }",
            "variables": { "search": title }
        }),
    };
    let mut response = agent()
        .post("https://graphql.anilist.co")
        .header("Accept", "application/json")
        .send_json(&body)
        .map_err(friendly)?;
    let status = response.status().as_u16();
    if status == 404 {
        return Ok(Artwork::default());
    }
    if !(200..300).contains(&status) {
        return Err(format!("AniList HTTP {status}"));
    }
    let value: serde_json::Value = response.body_mut().read_json().map_err(|e| e.to_string())?;
    Ok(Artwork {
        banner: value["data"]["Media"]["bannerImage"].as_str().map(str::to_string),
    })
}

pub mod parser {
    use super::*;

    fn re(pattern: &'static str) -> Regex {
        Regex::new(pattern).expect("valid pattern")
    }

    macro_rules! cached {
        ($name:ident, $pattern:expr) => {
            fn $name() -> &'static Regex {
                static CELL: OnceLock<Regex> = OnceLock::new();
                CELL.get_or_init(|| re($pattern))
            }
        };
    }

    cached!(series_id, r"/series/([0-9A-Z]{26})");
    cached!(cover_id, r"/cover/[a-z]+/([0-9A-Z]{26})\.");
    cached!(tip, r#"^[^>]*data-tip="([^"]*)""#);
    cached!(alt, r#"alt="([^"]*?) cover""#);
    cached!(chapter_label, r#"\.svg"[^>]*>\s*<span>([^<]+)</span>"#);
    cached!(chapter_id, r"/chapters/([0-9A-Z]{26})");
    cached!(datetime, r#"datetime="([^"]+)""#);
    cached!(year, r"<strong>Year:</strong>\s*<span>([^<]*)</span>");
    cached!(status, r"<strong>Status:</strong>\s*<span>([^<]*)</span>");
    cached!(kind, r"<strong>Type:</strong>\s*<span>([^<]*)</span>");
    cached!(h1, r"<h1[^>]*>([^<]+)</h1>");
    cached!(link_text, r"<a[^>]*>([^<]+)</a>");
    cached!(span_text, r"<span[^>]*>([^<]+)</span>");
    cached!(description, r"<strong>Description</strong>\s*<p[^>]*>([\s\S]*?)</p>");
    cached!(anilist, r"anilist\.co/manga/(\d+)");
    cached!(
        chapter_row,
        r#"href="(?:https://weebcentral\.com)?/chapters/([0-9A-Z]{26})"[^>]*>[\s\S]*?<span class="grow[^"]*">\s*<span[^>]*>([^<]*)</span>[\s\S]*?datetime="([^"]+)""#
    );
    cached!(page_img, r#"<img\s[^>]*?src="(https?://[^"]+)""#);
    cached!(numeric_entity, r"&#(x?)([0-9a-fA-F]+);");
    cached!(br, r"<br\s*/?>");

    fn first(regex: &Regex, text: &str) -> Option<String> {
        regex.captures(text).and_then(|c| c.get(1)).map(|m| m.as_str().to_string())
    }

    pub fn decode(text: &str) -> String {
        if !text.contains('&') {
            return text.to_string();
        }
        let numeric = numeric_entity().replace_all(text, |caps: &regex::Captures| {
            let radix = if caps[1].is_empty() { 10 } else { 16 };
            u32::from_str_radix(&caps[2], radix)
                .ok()
                .and_then(char::from_u32)
                .map(|c| c.to_string())
                .unwrap_or_else(|| caps[0].to_string())
        });
        numeric
            .replace("&quot;", "\"")
            .replace("&apos;", "'")
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&nbsp;", " ")
            .replace("&amp;", "&")
    }

    fn clean(text: &str) -> String {
        decode(text).split_whitespace().collect::<Vec<_>>().join(" ")
    }

    pub fn date(text: &str) -> Option<i64> {
        chrono::DateTime::parse_from_rfc3339(text).ok().map(|d| d.timestamp())
    }

    // every card is an <article>, desktop and mobile duplicates are merged
    pub fn cards(html: &str) -> Vec<MangaSummary> {
        let mut order: Vec<String> = Vec::new();
        let mut by_id: std::collections::HashMap<String, MangaSummary> = Default::default();

        for chunk in html.split("<article").skip(1) {
            let Some(id) = first(series_id(), chunk).or_else(|| first(cover_id(), chunk)) else {
                continue;
            };
            let manga = by_id.entry(id.clone()).or_insert_with(|| {
                order.push(id.clone());
                MangaSummary::new(id.clone(), "")
            });
            if manga.title.is_empty() {
                if let Some(title) = first(tip(), chunk).or_else(|| first(alt(), chunk)) {
                    manga.title = clean(&title);
                }
            }
            if manga.latest_chapter.is_none() {
                manga.latest_chapter = first(chapter_label(), chunk).map(|s| clean(&s));
            }
            if manga.latest_chapter_id.is_none() {
                manga.latest_chapter_id = first(chapter_id(), chunk);
            }
            if manga.updated_at.is_none() {
                manga.updated_at = first(datetime(), chunk).and_then(|d| date(&d));
            }
            if manga.year.is_none() {
                manga.year = first(year(), chunk).map(|s| clean(&s));
            }
            if manga.status.is_none() {
                manga.status = first(status(), chunk).map(|s| clean(&s));
            }
            if manga.kind.is_none() {
                manga.kind = first(kind(), chunk).map(|s| clean(&s));
            }
        }

        order
            .into_iter()
            .filter_map(|id| by_id.remove(&id))
            .filter(|m| !m.title.is_empty())
            .collect()
    }

    pub fn detail(html: &str, manga_id: &str) -> Option<MangaDetail> {
        let title = first(h1(), html).map(|t| clean(&t)).filter(|t| !t.is_empty())?;

        let field = |label: &str| -> Vec<String> {
            let pattern = format!(r"<strong>\s*{label}[^<]*</strong>([\s\S]*?)</li>");
            let Some(block) = Regex::new(&pattern).ok().and_then(|r| first(&r, html)) else {
                return Vec::new();
            };
            let mut values: Vec<String> = link_text().captures_iter(&block).map(|c| clean(&c[1])).collect();
            if values.is_empty() {
                values = span_text().captures_iter(&block).map(|c| clean(&c[1])).collect();
            }
            values.into_iter().filter(|v| !v.is_empty()).collect()
        };

        let summary = first(description(), html)
            .map(|raw| decode(&br().replace_all(&raw, "\n")).trim().to_string())
            .unwrap_or_default();

        Some(MangaDetail {
            id: manga_id.to_string(),
            title,
            authors: field("Author"),
            tags: field("Tag"),
            kind: field("Type").into_iter().next().unwrap_or_default(),
            status: field("Status").into_iter().next().unwrap_or_default(),
            released: field("Released").into_iter().next().unwrap_or_default(),
            official: field("Official Translation").first().map(|v| v == "Yes").unwrap_or(false),
            adult: field("Adult Content").first().map(|v| v == "Yes").unwrap_or(false),
            summary,
            anilist_id: first(anilist(), html).and_then(|v| v.parse().ok()),
        })
    }

    pub fn chapters(html: &str) -> Vec<Chapter> {
        let mut seen = std::collections::HashSet::new();
        chapter_row()
            .captures_iter(html)
            .filter(|c| seen.insert(c[1].to_string()))
            .map(|c| Chapter { id: c[1].to_string(), name: clean(&c[2]), date: date(&c[3]) })
            .collect()
    }

    // the same picture listed twice would show up twice, side by side in two-page mode
    pub fn pages(html: &str) -> Vec<String> {
        let mut seen = std::collections::HashSet::new();
        page_img().captures_iter(html).map(|c| decode(&c[1])).filter(|url| seen.insert(url.clone())).collect()
    }
}
