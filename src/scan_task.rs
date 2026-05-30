use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;

use rayon::prelude::*;

use crate::metadata;
use crate::search;

/// Messages streamed from the scan worker thread to the UI thread.
enum ScanUpdate {
    /// Total metadata-capable images discovered by the recursive directory walk.
    Total(usize),
    /// One image's tags finished loading.
    Loaded(PathBuf, Vec<String>),
    /// Every image has been processed.
    Done,
}

/// A recursive tag scan running on a background thread so the UI never blocks on
/// disk I/O. The UI polls [`ScanTask::drain`] once per frame to pull completed
/// work; results stream in incrementally and the window stays responsive even on
/// large folder trees. Dropping the task disconnects the channel, which makes the
/// worker stop at its next `send`.
pub struct ScanTask {
    rx: Receiver<ScanUpdate>,
    /// Number of images the walk found; `None` until the walk finishes.
    pub total: Option<usize>,
    /// Images whose tags have been loaded so far.
    pub loaded: usize,
    /// True once the worker reported completion (or the channel disconnected).
    pub done: bool,
}

impl ScanTask {
    pub fn spawn(base: PathBuf) -> Self {
        let (tx, rx) = mpsc::channel();
        thread::Builder::new()
            .name("tag_editor_scan".into())
            .spawn(move || {
                let images = search::collect_images_recursive(&base);
                if tx.send(ScanUpdate::Total(images.len())).is_err() {
                    return;
                }
                // Load every image's tags in parallel across rayon's thread pool.
                // Tag loading is per-file disk I/O plus parsing, so spreading it
                // over all cores overlaps the reads and cuts wall-clock time on
                // large trees. `mpsc` is multi-producer, so worker threads stream
                // results straight to the UI; `done_tx` outlives the parallel
                // section so the channel stays open until every load is sent.
                let done_tx = tx.clone();
                images.into_par_iter().for_each_with(tx, |tx, path| {
                    let tags = metadata::load_tags(&path);
                    let _ = tx.send(ScanUpdate::Loaded(path, tags));
                });
                let _ = done_tx.send(ScanUpdate::Done);
            })
            .expect("spawn scan thread");
        Self {
            rx,
            total: None,
            loaded: 0,
            done: false,
        }
    }

    /// Pulls every update currently queued without blocking, invoking `on_loaded`
    /// for each finished image. Returns true if any update was processed, so the
    /// caller knows whether to recompute its filtered view. Sets `self.done` when
    /// the scan completes or the worker thread goes away.
    pub fn drain(&mut self, mut on_loaded: impl FnMut(PathBuf, Vec<String>)) -> bool {
        let mut changed = false;
        loop {
            match self.rx.try_recv() {
                Ok(ScanUpdate::Total(n)) => {
                    self.total = Some(n);
                    changed = true;
                }
                Ok(ScanUpdate::Loaded(path, tags)) => {
                    self.loaded += 1;
                    on_loaded(path, tags);
                    changed = true;
                }
                Ok(ScanUpdate::Done) => {
                    self.done = true;
                    changed = true;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.done = true;
                    break;
                }
            }
        }
        changed
    }

    /// Human-readable progress, e.g. `"Scanning… 1234 / 5000"` (or just the loaded
    /// count while the walk is still discovering files).
    pub fn progress_label(&self) -> String {
        match self.total {
            Some(total) => format!("Scanning… {} / {}", self.loaded, total),
            None => format!("Scanning… {} found", self.loaded),
        }
    }
}
