// disk store for images and a gpu texture cache fed by decoder threads

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};

use egui::{ColorImage, Context, TextureHandle, TextureOptions, Vec2};
use sha2::{Digest, Sha256};

use crate::source;
use crate::store::{cache_dir, data_dir};

pub mod disk {
    use super::*;

    fn cache_folder() -> PathBuf {
        let dir = cache_dir().join("Images");
        let _ = fs::create_dir_all(&dir);
        dir
    }

    fn downloads_folder() -> PathBuf {
        let dir = data_dir().join("Downloads");
        let _ = fs::create_dir_all(&dir);
        dir
    }

    fn file_name(url: &str) -> String {
        let digest = Sha256::digest(url.as_bytes());
        digest.iter().map(|b| format!("{b:02x}")).collect()
    }

    // keep stores the file in the permanent downloads folder
    pub fn bytes(url: &str, keep: bool) -> Result<Vec<u8>, String> {
        if let Some(path) = url.strip_prefix("file:") {
            // ?rev only makes the cache key unique
            let path = path.split('?').next().unwrap_or(path);
            return fs::read(path).map_err(|e| e.to_string());
        }
        let name = file_name(url);
        let downloaded = downloads_folder().join(&name);
        let cached = cache_folder().join(&name);

        if let Ok(bytes) = fs::read(&downloaded) {
            return Ok(bytes);
        }
        if let Ok(bytes) = fs::read(&cached) {
            if keep {
                let _ = fs::write(&downloaded, &bytes);
            }
            return Ok(bytes);
        }
        // background previews come with the app's own name and no manga site referer
        let bytes = if crate::wallpapers::own_agent(url) { crate::wallpapers::fetch(url, false)? } else { source::fetch_bytes(url)? };
        let _ = fs::write(if keep { &downloaded } else { &cached }, &bytes);
        Ok(bytes)
    }

    pub fn remove_downloads(urls: &[String]) {
        let folder = downloads_folder();
        for url in urls {
            let _ = fs::remove_file(folder.join(file_name(url)));
        }
    }

    fn empty(dir: PathBuf) {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let _ = fs::remove_file(entry.path());
            }
        }
    }

    fn size(dir: PathBuf) -> u64 {
        fs::read_dir(dir)
            .map(|entries| entries.flatten().filter_map(|e| e.metadata().ok()).map(|m| m.len()).sum())
            .unwrap_or(0)
    }

    pub fn remove_all_downloads() {
        empty(downloads_folder());
    }

    pub fn clear_cache() {
        empty(cache_folder());
    }

    pub fn cache_size() -> u64 {
        size(cache_folder())
    }

    pub fn downloads_size() -> u64 {
        size(downloads_folder())
    }
}

pub fn format_bytes(bytes: u64) -> String {
    let units = ["bytes", "KB", "MB", "GB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1000.0 && unit < units.len() - 1 {
        value /= 1000.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} bytes")
    } else {
        format!("{value:.1} {}", units[unit])
    }
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum Kind {
    Cover,
    // full width, split into tiles when very tall
    Page,
    Blur,
    // banners and the wallpaper
    Wide,
}

impl Kind {
    fn max_width(self) -> u32 {
        match self {
            Kind::Cover => 420,
            Kind::Page => 1600,
            Kind::Blur => 64,
            // the background is kept sharp up to 4k
            Kind::Wide => 3840,
        }
    }
}

type Key = (String, Kind);

// gpus limit texture size, so tall webtoon strips are cut into tiles
const TILE_HEIGHT: usize = 4096;

struct Decoded {
    width: usize,
    height: usize,
    tiles: Vec<ColorImage>,
}

fn decode(bytes: &[u8], kind: Kind) -> Result<Decoded, String> {
    let image = image::load_from_memory(bytes).map_err(|e| e.to_string())?;
    let max_width = kind.max_width();
    let mut image = if image.width() > max_width {
        let height = (image.height() as f64 * max_width as f64 / image.width() as f64).round().max(1.0) as u32;
        image.resize_exact(max_width, height, image::imageops::FilterType::Triangle)
    } else {
        image
    };
    if kind == Kind::Blur {
        image = image.blur(4.0);
    }
    let rgba = image.to_rgba8();
    let (width, height) = (rgba.width() as usize, rgba.height() as usize);
    let raw = rgba.into_raw();

    let mut tiles = Vec::new();
    let mut y = 0;
    while y < height {
        let tile_height = TILE_HEIGHT.min(height - y);
        let start = y * width * 4;
        let end = (y + tile_height) * width * 4;
        tiles.push(ColorImage::from_rgba_unmultiplied([width, tile_height], &raw[start..end]));
        y += tile_height;
    }
    Ok(Decoded { width, height, tiles })
}

pub struct Loaded {
    pub size: Vec2,
    pub tiles: Vec<(TextureHandle, Vec2)>,
}

impl Loaded {
    pub fn aspect(&self) -> f32 {
        if self.size.y > 0.0 { self.size.x / self.size.y } else { 0.7 }
    }

    pub fn first(&self) -> &TextureHandle {
        &self.tiles[0].0
    }

    fn pixels(&self) -> usize {
        (self.size.x * self.size.y) as usize
    }
}

enum Entry {
    Loading,
    Ready(Loaded),
    Failed,
}

struct Slot {
    entry: Entry,
    last_used: u64,
}

pub enum State<'a> {
    Loading,
    Ready(&'a Loaded),
    Failed,
}

pub struct Images {
    slots: HashMap<Key, Slot>,
    jobs: Sender<Key>,
    results: Receiver<(Key, Result<Decoded, String>)>,
    frame: u64,
    // rough gpu memory budget in pixels
    budget: usize,
}

impl Images {
    pub fn new(ctx: &Context) -> Self {
        let (job_tx, job_rx) = channel::<Key>();
        let (result_tx, result_rx) = channel();
        let job_rx = Arc::new(Mutex::new(job_rx));

        for _ in 0..6 {
            let jobs = job_rx.clone();
            let results = result_tx.clone();
            let ctx = ctx.clone();
            std::thread::spawn(move || {
                loop {
                    let next = jobs.lock().ok().and_then(|rx| rx.recv().ok());
                    let Some(key) = next else { break };
                    let result = disk::bytes(&key.0, false).and_then(|bytes| decode(&bytes, key.1));
                    if results.send((key, result)).is_err() {
                        break;
                    }
                    ctx.request_repaint();
                }
            });
        }

        Self { slots: HashMap::new(), jobs: job_tx, results: result_rx, frame: 0, budget: 120_000_000 }
    }

    // call once per frame: uploads finished images and frees old ones
    pub fn begin_frame(&mut self, ctx: &Context) {
        self.frame += 1;
        while let Ok((key, result)) = self.results.try_recv() {
            let entry = match result {
                Ok(decoded) => {
                    let mut tiles = Vec::new();
                    for (index, tile) in decoded.tiles.into_iter().enumerate() {
                        let size = Vec2::new(tile.size[0] as f32, tile.size[1] as f32);
                        let handle = ctx.load_texture(format!("{}#{index}", key.0), tile, TextureOptions::LINEAR);
                        tiles.push((handle, size));
                    }
                    Entry::Ready(Loaded { size: Vec2::new(decoded.width as f32, decoded.height as f32), tiles })
                }
                Err(_) => Entry::Failed,
            };
            let last_used = self.slots.get(&key).map(|s| s.last_used).unwrap_or(self.frame);
            self.slots.insert(key, Slot { entry, last_used });
        }
        self.evict();
    }

    fn evict(&mut self) {
        let used: usize = self
            .slots
            .values()
            .map(|s| if let Entry::Ready(l) = &s.entry { l.pixels() } else { 0 })
            .sum();
        if used <= self.budget {
            return;
        }
        let mut candidates: Vec<(Key, u64, usize)> = self
            .slots
            .iter()
            .filter(|(_, s)| s.last_used + 2 < self.frame)
            .filter_map(|(k, s)| match &s.entry {
                Entry::Ready(l) => Some((k.clone(), s.last_used, l.pixels())),
                _ => None,
            })
            .collect();
        candidates.sort_by_key(|c| c.1);
        let mut over = used - self.budget;
        for (key, _, pixels) in candidates {
            if over == 0 {
                break;
            }
            self.slots.remove(&key);
            over = over.saturating_sub(pixels);
        }
    }

    pub fn get(&mut self, url: &str, kind: Kind) -> State<'_> {
        let key = (url.to_string(), kind);
        let frame = self.frame;
        if !self.slots.contains_key(&key) {
            self.slots.insert(key.clone(), Slot { entry: Entry::Loading, last_used: frame });
            let _ = self.jobs.send(key.clone());
        }
        let slot = self.slots.get_mut(&key).expect("just inserted");
        slot.last_used = frame;
        match &slot.entry {
            Entry::Loading => State::Loading,
            Entry::Ready(loaded) => State::Ready(loaded),
            Entry::Failed => State::Failed,
        }
    }

    pub fn prefetch(&mut self, url: &str, kind: Kind) {
        let _ = self.get(url, kind);
    }

    pub fn retry(&mut self, url: &str, kind: Kind) {
        self.slots.remove(&(url.to_string(), kind));
    }

    pub fn forget(&mut self, url: &str) {
        self.slots.retain(|k, _| k.0 != url);
    }

    pub fn clear(&mut self) {
        self.slots.clear();
    }

    pub fn retry_failed(&mut self) {
        let failed: HashSet<Key> = self
            .slots
            .iter()
            .filter(|(_, s)| matches!(s.entry, Entry::Failed))
            .map(|(k, _)| k.clone())
            .collect();
        for key in failed {
            self.slots.remove(&key);
        }
    }
}
