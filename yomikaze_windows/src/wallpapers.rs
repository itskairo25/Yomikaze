// premade backgrounds: anime wallpapers from wallhaven.
// the gallery shows previews, a picture is downloaded in full only when it's picked, and it
// stays in the data folder so it keeps working offline.

use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::OnceLock;
use std::time::Duration;

use sha2::{Digest, Sha256};

use crate::i18n::t;
use crate::net;
use crate::store::data_dir;

fn folder() -> PathBuf {
    let dir = data_dir().join("Wallpapers");
    let _ = fs::create_dir_all(&dir);
    dir
}

pub fn file_for(url: &str) -> PathBuf {
    let digest = Sha256::digest(url.as_bytes());
    let name: String = digest.iter().take(12).map(|b| format!("{b:02x}")).collect();
    folder().join(name)
}

// the saved picture as an image url the image loader understands
pub fn local_url(url: &str) -> String {
    format!("file:{}", file_for(url).display())
}

// pictures fetched with the app's own user agent instead of the manga site one
pub fn own_agent(url: &str) -> bool {
    ["https://th.wallhaven.cc/", "https://w.wallhaven.cc/"].iter().any(|host| url.starts_with(host))
}

fn agent() -> &'static ureq::Agent {
    static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
    AGENT.get_or_init(|| {
        ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(60)))
            .user_agent(concat!("Yomikaze/", env!("CARGO_PKG_VERSION"), " (manga reader for Windows)"))
            .http_status_as_error(false)
            .build()
            .into()
    })
}

// servers answer 429 when pictures are asked for too fast. patient waits as long as the
// server says and tries again, the gallery previews just give up and show a placeholder.
pub fn fetch(url: &str, patient: bool) -> Result<Vec<u8>, String> {
    if !net::is_online() {
        return Err(t("You're offline").to_string());
    }
    for _ in 0..if patient { 6 } else { 1 } {
        let mut response = agent().get(url).call().map_err(|e| e.to_string())?;
        let status = response.status().as_u16();
        if (status == 429 || status == 503) && patient {
            let wait = response
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.trim().parse::<u64>().ok())
                .unwrap_or(20)
                .clamp(5, 150);
            std::thread::sleep(Duration::from_secs(wait));
            continue;
        }
        if !(200..300).contains(&status) {
            return Err(format!("HTTP {status}"));
        }
        return response.body_mut().with_config().limit(96 * 1024 * 1024).read_to_vec().map_err(|e| e.to_string());
    }
    Err(t("The server is busy, try again in a minute.").to_string())
}

// big wallpapers are brought down to 4k and kept as jpeg, sharp on any screen without
// filling the disk with 20 mb pngs
fn download(url: &str) -> Result<(), String> {
    let bytes = fetch(url, true)?;
    let image = image::load_from_memory(&bytes).map_err(|e| e.to_string())?;
    if image.width() <= 3840 && image::guess_format(&bytes).is_ok_and(|f| f == image::ImageFormat::Jpeg) {
        return fs::write(file_for(url), bytes).map_err(|e| e.to_string());
    }
    let image = if image.width() > 3840 { image.resize(3840, 3840 * 4, image::imageops::FilterType::Lanczos3) } else { image };
    let mut out = std::io::Cursor::new(Vec::new());
    let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 92);
    image.to_rgb8().write_with_encoder(encoder).map_err(|e| e.to_string())?;
    fs::write(file_for(url), out.into_inner()).map_err(|e| e.to_string())
}

// anime wallpapers come from wallhaven: only the anime category, only safe for work, at least
// 1920x1080 and widescreen, so they fill the window sharply
#[derive(Clone)]
pub struct Wallpaper {
    pub full: String,
    pub thumb: String,
    pub resolution: String,
}

#[derive(Default)]
pub struct WallSearch {
    pub items: Vec<Wallpaper>,
    pub page: u32,
    pub last_page: u32,
    pub error: Option<String>,
    request: Option<poll_promise::Promise<Result<(Vec<Wallpaper>, u32), String>>>,
}

impl WallSearch {
    pub fn loading(&self) -> bool {
        self.request.is_some()
    }

    pub fn has_more(&self) -> bool {
        self.page < self.last_page
    }
}

// an empty query is the most liked anime wallpapers of the past year
fn search_wallhaven(query: &str, page: u32) -> Result<(Vec<Wallpaper>, u32), String> {
    if !net::is_online() {
        return Err(t("You're offline").to_string());
    }
    let mut request = agent()
        .get("https://wallhaven.cc/api/v1/search")
        .query("categories", "010")
        .query("purity", "100")
        .query("atleast", "1920x1080")
        .query("ratios", "16x9,16x10")
        .query("page", page.to_string());
    request = if query.is_empty() {
        request.query("sorting", "toplist").query("topRange", "1y")
    } else {
        request.query("sorting", "relevance").query("q", query)
    };
    let mut response = request.call().map_err(|e| e.to_string())?;
    let status = response.status().as_u16();
    if status == 429 {
        return Err(t("The server is busy, try again in a minute.").to_string());
    }
    if !(200..300).contains(&status) {
        return Err(format!("HTTP {status}"));
    }
    let body: serde_json::Value = response.body_mut().read_json().map_err(|e| e.to_string())?;
    let last_page = body["meta"]["last_page"].as_u64().unwrap_or(1) as u32;
    let items = body["data"]
        .as_array()
        .map(|list| {
            list.iter()
                .filter(|w| w["purity"].as_str() == Some("sfw"))
                .filter_map(|w| {
                    Some(Wallpaper {
                        full: w["path"].as_str()?.to_string(),
                        thumb: w["thumbs"]["large"].as_str()?.to_string(),
                        resolution: w["resolution"].as_str().unwrap_or("").replace('x', "×"),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    Ok((items, last_page))
}

// a series title the way wallpaper sites tag it: no subtitle, no year
pub fn search_name(title: &str) -> String {
    let short = title.split(':').next().unwrap_or(title);
    let short = short.split(" (").next().unwrap_or(short);
    short.trim().to_string()
}

// hand picked famous series for their own tab, written the way wallpaper sites tag them
pub const FAMOUS_SERIES: &[&str] = &[
    "Attack on Titan", "Demon Slayer", "Jujutsu Kaisen", "One Piece", "Naruto", "Bleach", "Dragon Ball",
    "Death Note", "Fullmetal Alchemist", "Hunter x Hunter", "My Hero Academia", "Chainsaw Man", "Spy x Family",
    "Frieren", "Solo Leveling", "Tokyo Ghoul", "One Punch Man", "Mob Psycho 100", "Sword Art Online", "Re:Zero",
    "Steins;Gate", "Code Geass", "Neon Genesis Evangelion", "Cowboy Bebop", "Your Name", "Weathering with You",
    "A Silent Voice", "Spirited Away", "Howl's Moving Castle", "Princess Mononoke", "Your Lie in April",
    "Violet Evergarden", "The Quintessential Quintuplets", "Kaguya-sama", "Rent-a-Girlfriend", "Horimiya",
    "Toradora", "Oshi no Ko", "Bocchi the Rock", "K-On", "My Dress-Up Darling", "Bunny Girl Senpai",
    "Lycoris Recoil", "Nisekoi", "Clannad", "Angel Beats", "Haikyuu", "Blue Lock", "Kuroko no Basket",
    "Vinland Saga", "Berserk", "JoJo's Bizarre Adventure", "Hellsing", "Black Clover", "Fire Force", "Dr. Stone",
    "The Promised Neverland", "Akame ga Kill", "Darling in the Franxx", "Kill la Kill", "Gurren Lagann",
    "Cyberpunk Edgerunners", "Made in Abyss", "Konosuba", "Overlord", "Tensei Shitara Slime Datta Ken",
    "Mushoku Tensei", "The Rising of the Shield Hero", "Fate", "Dandadan", "Kaiju No. 8", "Sakamoto Days",
    "Bungo Stray Dogs", "Noragami", "Assassination Classroom", "Tokyo Revengers", "Hell's Paradise",
    "Erased", "Parasyte", "Psycho-Pass", "Monogatari", "Ghost in the Shell", "Akira", "Boruto",
];

// the genre tabs list the best known series of each genre on anilist, asked for in one query
// and kept on disk for a week

pub const ANIME_GENRES: [&str; 16] = [
    "Action", "Adventure", "Comedy", "Drama", "Fantasy", "Romance", "Sci-Fi", "Slice of Life",
    "Sports", "Mystery", "Horror", "Supernatural", "Mecha", "Music", "Psychological", "Thriller",
];

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct AnimeBackground {
    pub title: String,
    pub url: String,
}

// the first set is the most popular series, then one per genre in ANIME_GENRES order
#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct AnimeSets {
    pub fetched_at: i64,
    pub sets: Vec<Vec<AnimeBackground>>,
}

fn anime_cache() -> PathBuf {
    data_dir().join("anime_backgrounds.json")
}

fn load_anime() -> Option<AnimeSets> {
    let sets: AnimeSets = serde_json::from_slice(&fs::read(anime_cache()).ok()?).ok()?;
    (sets.sets.len() == ANIME_GENRES.len() + 1).then_some(sets)
}

fn fetch_anime() -> Result<AnimeSets, String> {
    let fields = "id title { romaji english } bannerImage";
    let mut query = format!("query {{ popular: Page(perPage: 36) {{ media(type: ANIME, sort: POPULARITY_DESC, isAdult: false) {{ {fields} }} }}");
    for (index, genre) in ANIME_GENRES.iter().enumerate() {
        query.push_str(&format!(
            " g{index}: Page(perPage: 30) {{ media(type: ANIME, sort: POPULARITY_DESC, isAdult: false, genre: \"{genre}\") {{ {fields} }} }}"
        ));
    }
    query.push_str(" }");
    let mut response = agent()
        .post("https://graphql.anilist.co")
        .header("Accept", "application/json")
        .send_json(serde_json::json!({ "query": query }))
        .map_err(|e| e.to_string())?;
    let status = response.status().as_u16();
    if !(200..300).contains(&status) {
        return Err(format!("HTTP {status}"));
    }
    let body: serde_json::Value = response.body_mut().read_json().map_err(|e| e.to_string())?;
    let data = body.get("data").ok_or("AniList sent no data")?;
    // sequels look the same as the first season, only the first one stays
    let sequel = regex::Regex::new(r"(?i)season|part \d|\b\d+(st|nd|rd|th)\b|movie|film|recap|\b(2|3|ii|iii)$").expect("valid regex");
    let keys = std::iter::once("popular".to_string()).chain((0..ANIME_GENRES.len()).map(|i| format!("g{i}")));
    let mut sets = Vec::new();
    for key in keys {
        let mut seen = HashSet::new();
        let list = data[key.as_str()]["media"].as_array().cloned().unwrap_or_default();
        let set = list
            .iter()
            .filter_map(|m| {
                let url = m["bannerImage"].as_str()?.to_string();
                let title = m["title"]["english"].as_str().or(m["title"]["romaji"].as_str())?.to_string();
                Some(AnimeBackground { title, url })
            })
            .filter(|b| !sequel.is_match(&b.title) && seen.insert(b.url.clone()))
            .take(24)
            .collect();
        sets.push(set);
    }
    let sets = AnimeSets { fetched_at: crate::model::now(), sets };
    let _ = fs::write(anime_cache(), serde_json::to_vec(&sets).unwrap_or_default());
    Ok(sets)
}

pub enum Event {
    Saved(String),
    Failed(String, String),
}

// knows which pictures are on disk and downloads a picked one on a background thread
pub struct Gallery {
    saved: HashSet<String>,
    // links already looked for on disk
    checked: HashSet<String>,
    busy: HashSet<String>,
    anime: Option<AnimeSets>,
    anime_read: bool,
    anime_request: Option<poll_promise::Promise<Result<AnimeSets, String>>>,
    anime_error: Option<String>,
    // wallhaven results by search text, "" is the popular list
    searches: std::collections::HashMap<String, WallSearch>,
    sender: Sender<(String, Result<(), String>)>,
    receiver: Receiver<(String, Result<(), String>)>,
    ctx: egui::Context,
}

impl Gallery {
    pub fn new(ctx: &egui::Context) -> Self {
        let (sender, receiver) = channel();
        Self {
            saved: HashSet::new(),
            checked: HashSet::new(),
            busy: HashSet::new(),
            anime: None,
            anime_read: false,
            anime_request: None,
            anime_error: None,
            searches: Default::default(),
            sender,
            receiver,
            ctx: ctx.clone(),
        }
    }

    pub fn is_saved(&mut self, url: &str) -> bool {
        if self.saved.contains(url) {
            return true;
        }
        if self.checked.insert(url.to_string()) && file_for(url).exists() {
            self.saved.insert(url.to_string());
            return true;
        }
        false
    }

    // the anime sets from disk, asked for again from anilist when they're a week old
    pub fn anime(&mut self) -> Option<&AnimeSets> {
        if let Some(result) = self.anime_request.as_ref().and_then(|p| p.ready()) {
            match result {
                Ok(sets) => self.anime = Some(sets.clone()),
                Err(error) => self.anime_error = Some(error.clone()),
            }
            self.anime_request = None;
        }
        if !self.anime_read {
            self.anime_read = true;
            self.anime = load_anime();
        }
        let stale = self.anime.as_ref().is_none_or(|s| crate::model::now() - s.fetched_at > 7 * 24 * 60 * 60);
        if stale && self.anime_request.is_none() && self.anime_error.is_none() && net::is_online() {
            self.anime_request = Some(crate::app::spawn(&self.ctx, fetch_anime));
        }
        self.anime.as_ref()
    }

    pub fn anime_loading(&self) -> bool {
        self.anime_request.is_some()
    }

    pub fn anime_error(&self) -> Option<&str> {
        self.anime_error.as_deref()
    }

    pub fn retry_anime(&mut self) {
        self.anime_error = None;
    }

    pub fn is_busy(&self, url: &str) -> bool {
        self.busy.contains(url)
    }

    pub fn poll(&mut self) -> Vec<Event> {
        let mut events = Vec::new();
        while let Ok((url, result)) = self.receiver.try_recv() {
            self.busy.remove(&url);
            match result {
                Ok(()) => {
                    self.saved.insert(url.clone());
                    events.push(Event::Saved(url));
                }
                Err(error) => events.push(Event::Failed(url, error)),
            }
        }
        events
    }

    // the results for a search, the first page is asked for the first time it's looked at
    pub fn wallpapers(&mut self, query: &str) -> &WallSearch {
        let ctx = self.ctx.clone();
        let search = self.searches.entry(query.to_string()).or_default();
        if let Some(result) = search.request.as_ref().and_then(|p| p.ready()) {
            match result {
                Ok((items, last_page)) => {
                    search.items.extend(items.iter().cloned());
                    search.page += 1;
                    search.last_page = *last_page;
                }
                Err(error) => search.error = Some(error.clone()),
            }
            search.request = None;
        }
        if search.page == 0 && search.request.is_none() && search.error.is_none() && net::is_online() {
            let query = query.to_string();
            search.request = Some(crate::app::spawn(&ctx, move || search_wallhaven(&query, 1)));
        }
        search
    }

    pub fn more_wallpapers(&mut self, query: &str) {
        let ctx = self.ctx.clone();
        if let Some(search) = self.searches.get_mut(query) {
            if search.request.is_none() && search.has_more() {
                search.error = None;
                let (query, page) = (query.to_string(), search.page + 1);
                search.request = Some(crate::app::spawn(&ctx, move || search_wallhaven(&query, page)));
            }
        }
    }

    pub fn retry_wallpapers(&mut self, query: &str) {
        self.searches.remove(query);
    }

    pub fn download(&mut self, url: &str) {
        if self.saved.contains(url) || !self.busy.insert(url.to_string()) {
            return;
        }
        let (url, sender, ctx) = (url.to_string(), self.sender.clone(), self.ctx.clone());
        std::thread::spawn(move || {
            let result = download(&url);
            let _ = sender.send((url, result));
            ctx.request_repaint();
        });
    }
}
