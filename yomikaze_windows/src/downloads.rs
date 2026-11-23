// chapter downloads, queued so only a few run at once

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use egui::Context;

use crate::images::disk;
use crate::model::{Chapter, DownloadedChapter, now};
use crate::source;

const MAX_CONCURRENT: usize = 2;

#[derive(Clone)]
struct Job {
    chapter: Chapter,
    manga_id: String,
    manga_title: String,
}

#[derive(Default)]
struct Inner {
    pending: VecDeque<Job>,
    running: HashMap<String, (Arc<AtomicBool>, String)>,
    progress: HashMap<String, f32>,
    queued: HashSet<String>,
    failed: HashSet<String>,
    finished: Vec<DownloadedChapter>,
}

#[derive(Clone)]
pub struct DownloadManager {
    inner: Arc<Mutex<Inner>>,
    ctx: Context,
}

impl DownloadManager {
    pub fn new(ctx: &Context) -> Self {
        Self { inner: Arc::default(), ctx: ctx.clone() }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn progress(&self, chapter_id: &str) -> Option<f32> {
        self.lock().progress.get(chapter_id).copied()
    }

    pub fn is_queued(&self, chapter_id: &str) -> bool {
        self.lock().queued.contains(chapter_id)
    }

    pub fn has_failed(&self, chapter_id: &str) -> bool {
        self.lock().failed.contains(chapter_id)
    }

    pub fn is_busy(&self, chapter_id: &str) -> bool {
        let inner = self.lock();
        inner.progress.contains_key(chapter_id) || inner.queued.contains(chapter_id)
    }

    pub fn active_count(&self) -> usize {
        let inner = self.lock();
        inner.progress.len() + inner.queued.len()
    }

    // finished downloads since the last call
    pub fn take_finished(&self) -> Vec<DownloadedChapter> {
        std::mem::take(&mut self.lock().finished)
    }

    pub fn download(&self, chapters: &[Chapter], manga_id: &str, manga_title: &str) {
        {
            let mut inner = self.lock();
            for chapter in chapters {
                if inner.progress.contains_key(&chapter.id) || inner.queued.contains(&chapter.id) {
                    continue;
                }
                inner.failed.remove(&chapter.id);
                inner.queued.insert(chapter.id.clone());
                inner.pending.push_back(Job {
                    chapter: chapter.clone(),
                    manga_id: manga_id.to_string(),
                    manga_title: manga_title.to_string(),
                });
            }
        }
        self.pump();
    }

    fn pump(&self) {
        let mut inner = self.lock();
        while inner.running.len() < MAX_CONCURRENT {
            let Some(job) = inner.pending.pop_front() else { break };
            let id = job.chapter.id.clone();
            inner.queued.remove(&id);
            inner.progress.insert(id.clone(), 0.0);
            let cancel = Arc::new(AtomicBool::new(false));
            inner.running.insert(id.clone(), (cancel.clone(), job.manga_id.clone()));

            let manager = self.clone();
            std::thread::spawn(move || {
                let result = manager.run(&job, &cancel);
                {
                    let mut inner = manager.lock();
                    inner.running.remove(&id);
                    inner.progress.remove(&id);
                    match result {
                        Ok(Some(done)) => inner.finished.push(done),
                        Ok(None) => {}
                        Err(_) => {
                            inner.failed.insert(id.clone());
                        }
                    }
                }
                manager.pump();
                manager.ctx.request_repaint();
            });
        }
    }

    // Ok(None) means cancelled
    fn run(&self, job: &Job, cancel: &AtomicBool) -> Result<Option<DownloadedChapter>, String> {
        let pages = source::pages(&job.chapter.id)?;
        let mut done = 0;
        for batch in pages.chunks(4) {
            if cancel.load(Ordering::Relaxed) {
                return Ok(None);
            }
            let results: Vec<Result<Vec<u8>, String>> = std::thread::scope(|scope| {
                let handles: Vec<_> = batch.iter().map(|url| scope.spawn(move || disk::bytes(url, true))).collect();
                handles.into_iter().map(|h| h.join().unwrap_or_else(|_| Err("crashed".into()))).collect()
            });
            if let Some(Err(error)) = results.into_iter().find(|r| r.is_err()) {
                return Err(error);
            }
            done += batch.len();
            self.lock().progress.insert(job.chapter.id.clone(), done as f32 / pages.len() as f32);
            self.ctx.request_repaint();
        }
        if cancel.load(Ordering::Relaxed) {
            return Ok(None);
        }
        // keep the covers so the series looks right offline
        let _ = disk::bytes(&source::cover_url(&job.manga_id, false), true);
        let _ = disk::bytes(&source::cover_url(&job.manga_id, true), true);

        Ok(Some(DownloadedChapter {
            chapter_id: job.chapter.id.clone(),
            manga_id: job.manga_id.clone(),
            manga_title: job.manga_title.clone(),
            chapter_name: job.chapter.name.clone(),
            page_urls: pages,
            downloaded_at: now(),
        }))
    }

    pub fn cancel(&self, chapter_id: &str) {
        let mut inner = self.lock();
        if inner.queued.remove(chapter_id) {
            inner.pending.retain(|j| j.chapter.id != chapter_id);
        }
        if let Some((flag, _)) = inner.running.get(chapter_id) {
            flag.store(true, Ordering::Relaxed);
        }
    }

    pub fn cancel_series(&self, manga_id: &str) {
        let mut inner = self.lock();
        let waiting: Vec<String> = inner.pending.iter().filter(|j| j.manga_id == manga_id).map(|j| j.chapter.id.clone()).collect();
        inner.pending.retain(|j| j.manga_id != manga_id);
        for id in waiting {
            inner.queued.remove(&id);
        }
        for (flag, owner) in inner.running.values() {
            if owner == manga_id {
                flag.store(true, Ordering::Relaxed);
            }
        }
    }

    pub fn cancel_all(&self) {
        let mut inner = self.lock();
        inner.pending.clear();
        inner.queued.clear();
        for (flag, _) in inner.running.values() {
            flag.store(true, Ordering::Relaxed);
        }
    }
}
