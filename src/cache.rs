//! Result cache keyed by the md5 of the resolved wallpaper path.
//!
//! Mirrors `~/.cache/hyprlock-accent.json` semantics: full computed results
//! (accent/foreground/y_offset) are stored per wallpaper, plus manual
//! `--set-offset` overrides.

use md5::{Digest, Md5};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedEntry {
    pub wallpaper: String,
    pub accent: String,
    pub foreground: String,
    pub y_offset: i32,
    /// Source-file stamp for content invalidation (0 = unknown / legacy entry).
    #[serde(default)]
    src_mtime: u64,
    #[serde(default)]
    src_size: u64,
    /// Analysis parameters this result was computed with (0 = legacy/pin-only).
    #[serde(default)]
    max_width: u32,
    #[serde(default)]
    column_frac: f64,
}

impl CachedEntry {
    pub fn new(wallpaper: String, accent: String, foreground: String, y_offset: i32) -> Self {
        Self {
            wallpaper,
            accent,
            foreground,
            y_offset,
            src_mtime: 0,
            src_size: 0,
            max_width: 0,
            column_frac: 0.0,
        }
    }

    /// True when the entry holds computed colors produced with exactly these
    /// analysis parameters. Legacy and pin-only entries (zeroed params, empty
    /// colors) never match, so they fall through to a fresh compute while
    /// `cached_offset` still sees the pin.
    pub fn matches_analysis(&self, max_width: u32, column_frac: f64) -> bool {
        !self.accent.is_empty()
            && !self.foreground.is_empty()
            && self.max_width == max_width
            && self.column_frac == column_frac
    }
}

type CacheMap = BTreeMap<String, CachedEntry>;

fn cache_path() -> PathBuf {
    match std::env::var_os("XDG_CACHE_HOME") {
        Some(dir) => PathBuf::from(dir),
        None => {
            let home = std::env::var_os("HOME").unwrap_or_default();
            PathBuf::from(home).join(".cache")
        }
    }
    .join("hyprlock-accent.json")
}

/// (mtime nanos, size) of the wallpaper file; (0, 0) if unavailable.
/// Both values come from a single stat so they can never mix generations.
fn file_stamp(path: &str) -> (u64, u64) {
    match std::fs::metadata(path) {
        Ok(m) => (
            m.modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(0),
            m.len(),
        ),
        Err(_) => (0, 0),
    }
}

/// md5 of the resolved wallpaper path (Python uses `Path(wallpaper).resolve()`).
fn key_for(wallpaper: &str) -> String {
    let resolved = std::fs::canonicalize(wallpaper)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| wallpaper.to_string());
    format!("{:x}", Md5::digest(resolved.as_bytes()))
}

fn load_map() -> CacheMap {
    let path = cache_path();
    if !path.is_file() {
        return CacheMap::default();
    }
    let data = match std::fs::read_to_string(&path) {
        Ok(d) => d,
        Err(_) => return CacheMap::default(),
    };
    serde_json::from_str(&data).unwrap_or_default()
}

/// Write via temp file + rename so a concurrent run can never see a
/// half-written JSON (which `unwrap_or_default` would silently wipe).
fn save_map(map: &CacheMap) {
    let path = cache_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let Ok(data) = serde_json::to_string_pretty(map) else {
        return;
    };
    let tmp = path.with_extension("json.tmp");
    if std::fs::write(&tmp, data).is_ok() {
        let _ = std::fs::rename(&tmp, &path);
    }
}

/// Cached entry for a wallpaper path, if present AND the source file still
/// matches the stored stamp (same-path wallpaper swaps are recomputed).
pub fn load_cached(wallpaper: &str) -> Option<CachedEntry> {
    let entry = load_map().get(&key_for(wallpaper)).cloned()?;
    let stamp = file_stamp(wallpaper);
    (entry.src_mtime == stamp.0 && entry.src_size == stamp.1).then_some(entry)
}

/// Manually pinned y_offset override for a wallpaper, else `computed`.
pub fn cached_offset(wallpaper: &str, computed: i32) -> i32 {
    load_cached(wallpaper)
        .map(|e| e.y_offset)
        .unwrap_or(computed)
}

/// Persist a manual offset override for the current wallpaper.
pub fn pin_offset(wallpaper: &str, value: i32) {
    let mut map = load_map();
    let key = key_for(wallpaper);
    let (src_mtime, src_size) = file_stamp(wallpaper);
    match map.get_mut(&key) {
        Some(entry) => {
            entry.y_offset = value;
            entry.src_mtime = src_mtime;
            entry.src_size = src_size;
        }
        None => {
            map.insert(
                key,
                CachedEntry {
                    wallpaper: wallpaper.to_string(),
                    accent: String::new(),
                    foreground: String::new(),
                    y_offset: value,
                    src_mtime,
                    src_size,
                    max_width: 0,
                    column_frac: 0.0,
                },
            );
        }
    }
    save_map(&map);
}

/// Store the full computed result, stamped and tagged with the analysis
/// parameters it was produced with.
pub fn store(wallpaper: &str, entry: CachedEntry, analysis: (u32, f64)) {
    let mut map = load_map();
    let (src_mtime, src_size) = file_stamp(wallpaper);
    let entry = CachedEntry {
        src_mtime,
        src_size,
        max_width: analysis.0,
        column_frac: analysis.1,
        ..entry
    };
    map.insert(key_for(wallpaper), entry);
    save_map(&map);
}
