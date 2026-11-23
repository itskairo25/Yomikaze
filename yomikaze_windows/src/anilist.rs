// anilist sync: login through the yomikaze:// link, importing the manga list, and pushing the
// library and reading progress back. nothing is ever deleted on anilist.
// api rules: https://anilist.gitbook.io/anilist-apiv2-docs (currently 30 requests per minute)
//
// a full sync reads the whole list and runs at start, every 15 minutes and on "sync now".
// changes in the app are pushed right away for just the series that changed, using what the
// last full sync saw as the anilist side.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::i18n::t;
use crate::model::{Chapter, MangaDetail};
use crate::source::{self, SearchSort};
use crate::store::{self, catalog};

// your own client from https://anilist.co/settings/developer with the redirect url
// yomikaze://anilist, given at build time: YOMIKAZE_ANILIST_CLIENT_ID=12345 cargo build
pub const CLIENT_ID: &str = match option_env!("YOMIKAZE_ANILIST_CLIENT_ID") {
    Some(id) => id,
    None => "",
};
const API: &str = "https://graphql.anilist.co";
// stays under the 30 requests per minute limit
const API_GAP: Duration = Duration::from_millis(2100);
// gentle on weebcentral while matching series
const SITE_GAP: Duration = Duration::from_millis(1200);

pub fn configured() -> bool {
    !CLIENT_ID.is_empty()
}

pub fn login_url() -> String {
    format!("https://anilist.co/api/v2/oauth/authorize?client_id={CLIENT_ID}&response_type=token")
}

pub fn manga_url(id: i64) -> String {
    format!("https://anilist.co/manga/{id}")
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct User {
    pub id: i64,
    pub name: String,
}

// one series as the sync sees it
#[derive(Clone, Debug)]
pub struct Local {
    pub manga_id: String,
    pub title: String,
    pub in_library: bool,
    pub anilist_id: Option<i64>,
    // chapters read: the highest finished chapter number, or the one before a started chapter
    pub progress: i64,
    // anything opened at all
    pub started: bool,
}

#[derive(Clone, Debug)]
pub struct Imported {
    pub detail: MangaDetail,
    pub anilist_id: i64,
}

// anilist is further than the app: these chapters get marked as read
#[derive(Clone, Debug)]
pub struct RemoteReads {
    pub manga_id: String,
    pub progress: i64,
    pub chapters: Vec<Chapter>,
    // unix seconds of the anilist update, keeps the history order sensible
    pub updated_at: i64,
}

#[derive(Default)]
pub struct Outcome {
    pub full: bool,
    // manga id and its anilist id, found during the sync
    pub links: Vec<(String, i64)>,
    pub reads: Vec<RemoteReads>,
    pub imported: Vec<Imported>,
    // anilist ids that couldn't be found on weebcentral
    pub unmatched: Vec<i64>,
    // series that were just moved to completed: (title, anilist id)
    pub finished: Vec<(String, i64)>,
}

pub enum Event {
    LoggedIn { token: String, user: User },
    LoginFailed(String),
    Synced(Outcome),
    TokenRejected,
}

#[derive(Clone, Default)]
pub struct Status {
    pub busy: bool,
    pub message: String,
    pub error: Option<String>,
    pub favourites: HashSet<i64>,
}

enum Job {
    Login(String),
    Sync { token: String, user: User, local: Vec<Local>, skip: HashSet<i64> },
    Push { token: String, user: User, local: Vec<Local> },
    Favourite { token: String, id: i64 },
}

#[derive(Clone)]
pub struct AniList {
    jobs: Sender<Job>,
    status: Arc<Mutex<Status>>,
    events: Arc<Mutex<Vec<Event>>>,
}

impl AniList {
    pub fn start(ctx: &egui::Context) -> Self {
        let (jobs, inbox) = channel();
        let client = Self { jobs, status: Arc::default(), events: Arc::default() };
        let worker = Worker {
            client: client.clone(),
            ctx: ctx.clone(),
            last_api: None,
            last_site: None,
            remote: None,
        };
        std::thread::spawn(move || worker.run(inbox));
        client
    }

    pub fn status(&self) -> Status {
        self.status.lock().map(|s| s.clone()).unwrap_or_default()
    }

    pub fn take_events(&self) -> Vec<Event> {
        self.events.lock().map(|mut e| std::mem::take(&mut *e)).unwrap_or_default()
    }

    pub fn login(&self, token: String) {
        let _ = self.jobs.send(Job::Login(token));
    }

    fn mark_busy(&self) -> bool {
        match self.status.lock() {
            Ok(mut status) if !status.busy => {
                status.busy = true;
                true
            }
            _ => false,
        }
    }

    pub fn sync(&self, token: &str, user: &User, local: Vec<Local>, skip: HashSet<i64>) {
        if self.mark_busy() {
            let _ = self.jobs.send(Job::Sync { token: token.to_string(), user: user.clone(), local, skip });
        }
    }

    // pushes just these series, queued behind a running sync if there is one
    pub fn push(&self, token: &str, user: &User, local: Vec<Local>) {
        let _ = self.jobs.send(Job::Push { token: token.to_string(), user: user.clone(), local });
    }

    pub fn toggle_favourite(&self, token: &str, id: i64) {
        if let Ok(mut status) = self.status.lock() {
            if !status.favourites.remove(&id) {
                status.favourites.insert(id);
            }
        }
        let _ = self.jobs.send(Job::Favourite { token: token.to_string(), id });
    }

    pub fn clear(&self) {
        if let Ok(mut status) = self.status.lock() {
            *status = Status::default();
        }
    }
}

struct Worker {
    client: AniList,
    ctx: egui::Context,
    last_api: Option<Instant>,
    last_site: Option<Instant>,
    // the anilist list as the last full sync saw it, kept up to date by pushes
    remote: Option<HashMap<i64, Entry>>,
}

enum ApiError {
    Rejected,
    Other(String),
}

#[derive(Clone)]
struct Entry {
    status: String,
    progress: i64,
    updated_at: i64,
    titles: Vec<String>,
}

impl Worker {
    fn run(mut self, inbox: Receiver<Job>) {
        for job in inbox {
            match job {
                Job::Login(token) => {
                    self.remote = None;
                    let event = match self.viewer(&token) {
                        Ok(user) => Event::LoggedIn { token, user },
                        Err(ApiError::Rejected) => Event::LoginFailed(t("AniList didn't accept the login.").to_string()),
                        Err(ApiError::Other(error)) => Event::LoginFailed(error),
                    };
                    self.emit(event);
                }
                Job::Sync { token, user, local, skip } => {
                    let result = self.full_sync(&token, &user, local, skip);
                    self.finish(result);
                }
                Job::Push { token, user, local } => {
                    let result = if self.remote.is_none() {
                        self.full_sync(&token, &user, local, HashSet::new())
                    } else {
                        self.push(&token, local)
                    };
                    self.finish(result);
                }
                Job::Favourite { token, id } => {
                    let query = "mutation ($id: Int) { ToggleFavourite(mangaId: $id) { manga { pageInfo { total } } } }";
                    if let Err(ApiError::Rejected) = self.api(&token, query, json!({ "id": id })) {
                        self.emit(Event::TokenRejected);
                    }
                }
            }
        }
    }

    fn finish(&mut self, result: Result<Outcome, ApiError>) {
        if let Ok(mut status) = self.client.status.lock() {
            status.busy = false;
            status.message.clear();
            status.error = match &result {
                Err(ApiError::Other(error)) => Some(error.clone()),
                _ => None,
            };
        }
        match result {
            Ok(outcome) => self.emit(Event::Synced(outcome)),
            Err(ApiError::Rejected) => {
                self.remote = None;
                self.emit(Event::TokenRejected);
            }
            Err(ApiError::Other(_)) => self.ctx.request_repaint(),
        }
    }

    fn emit(&self, event: Event) {
        if let Ok(mut events) = self.client.events.lock() {
            events.push(event);
        }
        self.ctx.request_repaint();
    }

    fn set_message(&self, message: String) {
        if let Ok(mut status) = self.client.status.lock() {
            status.message = message;
        }
        self.ctx.request_repaint();
    }

    fn wait(last: &mut Option<Instant>, gap: Duration) {
        if let Some(at) = *last {
            let since = at.elapsed();
            if since < gap {
                std::thread::sleep(gap - since);
            }
        }
        *last = Some(Instant::now());
    }

    fn api(&mut self, token: &str, query: &str, variables: Value) -> Result<Value, ApiError> {
        for _ in 0..3 {
            Self::wait(&mut self.last_api, API_GAP);
            let agent: ureq::Agent = ureq::Agent::config_builder()
                .timeout_global(Some(Duration::from_secs(30)))
                .http_status_as_error(false)
                .build()
                .into();
            let mut response = agent
                .post(API)
                .header("Authorization", &format!("Bearer {token}"))
                .header("Accept", "application/json")
                .send_json(json!({ "query": query, "variables": variables }))
                .map_err(|_| ApiError::Other(t("Couldn't reach AniList.").to_string()))?;
            let status = response.status().as_u16();
            if status == 429 {
                let wait = response
                    .headers()
                    .get("Retry-After")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(60)
                    .min(90);
                std::thread::sleep(Duration::from_secs(wait));
                continue;
            }
            let body: Value = response.body_mut().read_json().unwrap_or(Value::Null);
            if status == 401 || (status == 400 && body.to_string().contains("Invalid token")) {
                return Err(ApiError::Rejected);
            }
            if !(200..300).contains(&status) {
                let message = body["errors"][0]["message"].as_str().unwrap_or("unknown error");
                return Err(ApiError::Other(format!("AniList: {message}")));
            }
            return Ok(body["data"].clone());
        }
        Err(ApiError::Other(t("AniList is busy, try again in a minute.").to_string()))
    }

    fn viewer(&mut self, token: &str) -> Result<User, ApiError> {
        let data = self.api(token, "query { Viewer { id name } }", json!({}))?;
        let id = data["Viewer"]["id"].as_i64().ok_or(ApiError::Rejected)?;
        Ok(User { id, name: data["Viewer"]["name"].as_str().unwrap_or_default().to_string() })
    }

    fn list(&mut self, token: &str, user: &User) -> Result<HashMap<i64, Entry>, ApiError> {
        let query = "query ($userId: Int) { MediaListCollection(userId: $userId, type: MANGA) { lists { entries { \
                     mediaId status progress updatedAt media { synonyms title { romaji english native } } } } } }";
        let data = self.api(token, query, json!({ "userId": user.id }))?;
        let mut entries = HashMap::new();
        for list in data["MediaListCollection"]["lists"].as_array().into_iter().flatten() {
            for entry in list["entries"].as_array().into_iter().flatten() {
                let Some(id) = entry["mediaId"].as_i64() else { continue };
                let media = &entry["media"];
                let mut titles: Vec<String> = ["english", "romaji", "native"]
                    .iter()
                    .filter_map(|k| media["title"][k].as_str().map(str::to_string))
                    .collect();
                titles.extend(media["synonyms"].as_array().into_iter().flatten().filter_map(|s| s.as_str().map(str::to_string)));
                entries.insert(id, Entry {
                    status: entry["status"].as_str().unwrap_or("CURRENT").to_string(),
                    progress: entry["progress"].as_i64().unwrap_or(0),
                    updated_at: entry["updatedAt"].as_i64().unwrap_or(0),
                    titles,
                });
            }
        }
        Ok(entries)
    }

    fn favourites(&mut self, token: &str, user: &User) -> Result<HashSet<i64>, ApiError> {
        let query = "query ($userId: Int, $page: Int) { User(id: $userId) { favourites { manga(page: $page, perPage: 50) { \
                     nodes { id } pageInfo { hasNextPage } } } } }";
        let mut ids = HashSet::new();
        for page in 1..=10 {
            let data = self.api(token, query, json!({ "userId": user.id, "page": page }))?;
            let manga = &data["User"]["favourites"]["manga"];
            ids.extend(manga["nodes"].as_array().into_iter().flatten().filter_map(|n| n["id"].as_i64()));
            if !manga["pageInfo"]["hasNextPage"].as_bool().unwrap_or(false) {
                break;
            }
        }
        Ok(ids)
    }

    fn save_entry(&mut self, token: &str, id: i64, status: &str, progress: i64) -> Result<(), ApiError> {
        let query = "mutation ($mediaId: Int, $status: MediaListStatus, $progress: Int) { \
                     SaveMediaListEntry(mediaId: $mediaId, status: $status, progress: $progress) { id } }";
        self.api(token, query, json!({ "mediaId": id, "status": status, "progress": progress }))?;
        if let Some(remote) = &mut self.remote {
            let entry = remote.entry(id).or_insert_with(|| Entry {
                status: String::new(),
                progress: 0,
                updated_at: 0,
                titles: Vec::new(),
            });
            entry.status = status.to_string();
            entry.progress = progress;
            entry.updated_at = crate::model::now();
        }
        Ok(())
    }

    // series details come from the catalog cache when possible, otherwise from weebcentral
    fn detail(&mut self, manga_id: &str) -> Option<MangaDetail> {
        if let Some(detail) = catalog::load::<MangaDetail>(&catalog::detail_name(manga_id)) {
            return Some(detail);
        }
        Self::wait(&mut self.last_site, SITE_GAP);
        let detail = source::detail(manga_id).ok()?;
        catalog::save(&catalog::detail_name(manga_id), &detail);
        Some(detail)
    }

    fn chapters(&mut self, manga_id: &str, fetch: bool) -> Vec<Chapter> {
        let cached = catalog::chapters(manga_id);
        if !cached.is_empty() || !fetch {
            return cached;
        }
        Self::wait(&mut self.last_site, SITE_GAP);
        let fresh = source::chapters(manga_id).unwrap_or_default();
        if !fresh.is_empty() {
            catalog::save(&catalog::chapters_name(manga_id), &fresh);
        }
        fresh
    }

    fn anilist_id_for(&mut self, token: &str, local: &Local) -> Option<i64> {
        if let Some(id) = self.detail(&local.manga_id).and_then(|d| d.anilist_id) {
            return Some(id);
        }
        let query = "query ($search: String) { Media(search: $search, type: MANGA) { id } }";
        self.api(token, query, json!({ "search": local.title })).ok()?["Media"]["id"].as_i64()
    }

    // finds the weebcentral series for an anilist entry
    fn find_on_site(&mut self, id: i64, entry: &Entry) -> Option<MangaDetail> {
        let wanted: Vec<String> = entry.titles.iter().map(|t| normalize(t)).collect();
        for title in entry.titles.iter().take(2) {
            Self::wait(&mut self.last_site, SITE_GAP);
            let Ok(page) = source::search(title, SearchSort::BestMatch, None, 0, true) else { continue };
            for card in page.items.iter().take(3) {
                let Some(detail) = self.detail(&card.id) else { continue };
                let same_title = wanted.contains(&normalize(&detail.title));
                if detail.anilist_id == Some(id) || (detail.anilist_id.is_none() && same_title) {
                    return Some(detail);
                }
            }
        }
        None
    }

    // true when the series has ended and the last chapter is read
    fn finished(&mut self, local: &Local) -> bool {
        let complete = self.detail(&local.manga_id).is_some_and(|d| d.status.to_lowercase().contains("complet"));
        if !complete || local.progress <= 0 {
            return false;
        }
        let last = self.chapters(&local.manga_id, false).iter().filter_map(|c| chapter_number(&c.name)).max();
        last.is_some_and(|last| local.progress >= last)
    }

    fn chapters_up_to(&mut self, manga_id: &str, progress: i64) -> Vec<Chapter> {
        self.chapters(manga_id, true)
            .into_iter()
            .filter(|c| chapter_number(&c.name).is_some_and(|n| n <= progress))
            .collect()
    }

    // brings one series in line with anilist, both ways
    fn reconcile(&mut self, token: &str, local: &Local, outcome: &mut Outcome) -> Result<Option<i64>, ApiError> {
        let id = match local.anilist_id {
            Some(id) => id,
            None if local.in_library || local.started => match self.anilist_id_for(token, local) {
                Some(id) => {
                    outcome.links.push((local.manga_id.clone(), id));
                    id
                }
                None => return Ok(None),
            },
            None => return Ok(None),
        };
        let remote = self.remote.as_ref().and_then(|r| r.get(&id)).cloned();

        // anilist is further: bring those chapters into the app
        if let Some(remote) = &remote {
            if remote.progress > local.progress {
                let chapters = self.chapters_up_to(&local.manga_id, remote.progress);
                outcome.reads.push(RemoteReads {
                    manga_id: local.manga_id.clone(),
                    progress: remote.progress,
                    chapters,
                    updated_at: remote.updated_at,
                });
                return Ok(Some(id));
            }
        }

        let finished = self.finished(local);
        let status = match &remote {
            _ if finished => "COMPLETED".to_string(),
            Some(r) if r.status == "PLANNING" && local.started => "CURRENT".to_string(),
            Some(r) => r.status.clone(),
            None if local.started => "CURRENT".to_string(),
            None => "PLANNING".to_string(),
        };
        let changed = match &remote {
            None => local.in_library || local.started,
            Some(r) => local.progress > r.progress || status != r.status,
        };
        if changed {
            self.save_entry(token, id, &status, local.progress)?;
            if finished && remote.as_ref().is_none_or(|r| r.status != "COMPLETED") {
                outcome.finished.push((local.title.clone(), id));
            }
        }
        Ok(Some(id))
    }

    fn push(&mut self, token: &str, local: Vec<Local>) -> Result<Outcome, ApiError> {
        let mut outcome = Outcome::default();
        for series in &local {
            self.reconcile(token, series, &mut outcome)?;
        }
        Ok(outcome)
    }

    fn full_sync(&mut self, token: &str, user: &User, local: Vec<Local>, skip: HashSet<i64>) -> Result<Outcome, ApiError> {
        let mut outcome = Outcome { full: true, ..Outcome::default() };
        self.set_message(String::new());
        self.remote = Some(self.list(token, user)?);
        let favourites = self.favourites(token, user)?;
        if let Ok(mut status) = self.client.status.lock() {
            status.favourites = favourites;
        }

        let mut linked: HashSet<i64> = HashSet::new();
        for series in &local {
            if let Some(id) = self.reconcile(token, series, &mut outcome)? {
                linked.insert(id);
            }
        }

        // bring in what is only on anilist
        let remote = self.remote.clone().unwrap_or_default();
        let mut missing: Vec<i64> = remote.keys().copied().filter(|id| !linked.contains(id) && !skip.contains(id)).collect();
        missing.sort_unstable();
        for (index, id) in missing.iter().enumerate() {
            self.set_message(format!("{}/{}", index + 1, missing.len()));
            let entry = &remote[id];
            match self.find_on_site(*id, entry) {
                Some(detail) => {
                    if entry.progress > 0 {
                        let chapters = self.chapters_up_to(&detail.id, entry.progress);
                        outcome.reads.push(RemoteReads {
                            manga_id: detail.id.clone(),
                            progress: entry.progress,
                            chapters,
                            updated_at: entry.updated_at,
                        });
                    }
                    outcome.imported.push(Imported { detail, anilist_id: *id });
                }
                None => outcome.unmatched.push(*id),
            }
        }
        Ok(outcome)
    }
}

fn normalize(title: &str) -> String {
    title.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect()
}

// "Chapter 112" or "Episode 10.5" gives 112 or 10
pub fn chapter_number(name: &str) -> Option<i64> {
    name.split_whitespace().rev().find_map(|word| word.parse::<f64>().ok()).map(|n| n.floor() as i64)
}

// login link handling. the browser opens yomikaze://anilist#access_token=..., windows starts
// a second yomikaze with that link, which leaves the token here for the running app.

fn pending_path() -> PathBuf {
    store::data_dir().join("anilist-login.bin")
}

fn token_path() -> PathBuf {
    store::data_dir().join("anilist.bin")
}

pub fn receive_link(link: &str) {
    let fragment = link.split_once('#').map(|(_, f)| f).unwrap_or_default();
    let token = fragment
        .split('&')
        .find_map(|pair| pair.strip_prefix("access_token="))
        .unwrap_or_default();
    if !token.is_empty() {
        let _ = std::fs::write(pending_path(), crate::hello::protect(token.as_bytes()));
    }
}

pub fn take_pending_login() -> Option<String> {
    let bytes = std::fs::read(pending_path()).ok()?;
    let _ = std::fs::remove_file(pending_path());
    String::from_utf8(crate::hello::unprotect(&bytes)?).ok()
}

pub fn save_token(token: &str) {
    let _ = std::fs::write(token_path(), crate::hello::protect(token.as_bytes()));
}

pub fn load_token() -> Option<String> {
    let bytes = std::fs::read(token_path()).ok()?;
    String::from_utf8(crate::hello::unprotect(&bytes)?).ok()
}

pub fn delete_token() {
    let _ = std::fs::remove_file(token_path());
    let _ = std::fs::remove_file(pending_path());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chapter_numbers() {
        assert_eq!(chapter_number("Chapter 1198"), Some(1198));
        assert_eq!(chapter_number("Episode 10.5"), Some(10));
        assert_eq!(chapter_number("Oneshot"), None);
    }

    #[test]
    fn titles_compare_loosely() {
        assert_eq!(normalize("Solo Leveling!"), normalize("solo leveling"));
    }
}
