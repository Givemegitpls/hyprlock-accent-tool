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

fn save_map(map: &CacheMap) {
    let path = cache_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(data) = serde_json::to_string_pretty(map) {
        let _ = std::fs::write(path, data);
    }
}

/// Full cached entry for a wallpaper path, if present.
pub fn load_cached(wallpaper: &str) -> Option<CachedEntry> {
    load_map().remove(&key_for(wallpaper))
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
    match map.get_mut(&key) {
        Some(entry) => entry.y_offset = value,
        None => {
            map.insert(
                key,
                CachedEntry {
                    wallpaper: wallpaper.to_string(),
                    accent: String::new(),
                    foreground: String::new(),
                    y_offset: value,
                },
            );
        }
    }
    save_map(&map);
}

/// Store the full computed result.
pub fn store(wallpaper: &str, entry: CachedEntry) {
    let mut map = load_map();
    map.insert(key_for(wallpaper), entry);
    save_map(&map);
}
