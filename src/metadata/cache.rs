use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

/// One cached file's tags plus the modification time they were read at. A cache
/// hit requires the file's current mtime to still match, so edits made outside
/// the app invalidate the entry automatically.
#[derive(Serialize, Deserialize, Clone)]
struct Entry {
    mtime: Option<SystemTime>,
    tags: Vec<String>,
}

/// Process-wide tag cache. The desktop app and its embedded web server run in the
/// same process, so both share this map for free. Loaded from disk on first use.
static CACHE: OnceLock<Mutex<HashMap<PathBuf, Entry>>> = OnceLock::new();

fn cache() -> &'static Mutex<HashMap<PathBuf, Entry>> {
    CACHE.get_or_init(|| Mutex::new(load_from_disk()))
}

fn cache_path() -> Option<PathBuf> {
    dirs::cache_dir().map(|d| d.join("tag_editor").join("tag_cache.json"))
}

fn load_from_disk() -> HashMap<PathBuf, Entry> {
    let Some(path) = cache_path() else {
        return HashMap::new();
    };
    let Ok(bytes) = fs::read(&path) else {
        return HashMap::new();
    };
    serde_json::from_slice(&bytes).unwrap_or_default()
}

fn mtime_of(path: &Path) -> Option<SystemTime> {
    fs::metadata(path).ok()?.modified().ok()
}

/// Returns cached tags when the file's mtime still matches the cached entry.
/// Otherwise runs `load`, stores the result under the current mtime, and returns
/// it. A cache hit costs a single `stat` instead of opening and parsing the file.
pub fn get_or_load(path: &Path, load: impl FnOnce(&Path) -> Vec<String>) -> Vec<String> {
    let mtime = mtime_of(path);
    if mtime.is_some() {
        let map = cache().lock().expect("tag cache poisoned");
        if let Some(entry) = map.get(path) {
            if entry.mtime == mtime {
                return entry.tags.clone();
            }
        }
    }
    let tags = load(path);
    let mut map = cache().lock().expect("tag cache poisoned");
    map.insert(
        path.to_path_buf(),
        Entry {
            mtime,
            tags: tags.clone(),
        },
    );
    tags
}

/// Drops any cached entry for `path`. Called after a write so the next read pulls
/// the freshly-saved tags rather than a stale cached copy.
pub fn invalidate(path: &Path) {
    cache().lock().expect("tag cache poisoned").remove(path);
}

/// Persists the cache to disk. Meant to be called after a scan finishes and on
/// app exit; failures are ignored since the cache is only an optimization.
pub fn flush() {
    let Some(path) = cache_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let map = cache().lock().expect("tag cache poisoned");
    if let Ok(json) = serde_json::to_vec(&*map) {
        let _ = fs::write(&path, json);
    }
}
