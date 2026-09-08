//! Wallpaper acquisition via `awww query`.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;

/// One output's wallpaper as reported by awww.
#[derive(Debug, Clone)]
pub struct MonitorWallpaper {
    /// Output port name (e.g. `HDMI-A-1`), `None` when it could not be parsed.
    pub output: Option<String>,
    pub path: PathBuf,
}

/// Return the current awww wallpaper per output.
///
/// `HYPRLOCK_WALLPAPER` overrides the query (testing / fixed wallpapers) and
/// yields a single unnamed entry.
pub fn get_wallpapers() -> Result<Vec<MonitorWallpaper>, String> {
    if let Some(forced) = std::env::var_os("HYPRLOCK_WALLPAPER") {
        let p = Path::new(&forced);
        if !p.is_file() {
            return Err(format!("wallpaper does not exist: {}", p.display()));
        }
        return Ok(vec![MonitorWallpaper {
            output: None,
            path: p.to_path_buf(),
        }]);
    }

    let out = Command::new("awww")
        .args(["query"])
        .output()
        .map_err(|e| format!("awww not found or failed: {e}"))?;

    let stdout = String::from_utf8_lossy(&out.stdout);
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(format!("awww query failed: {}", stderr.trim()));
    }

    // Line format: `: HDMI-A-1: 1920x1080, scale: 1, currently displaying: image: /path`
    let mut result = Vec::new();
    for line in stdout.lines() {
        let Some((head, path)) = line.split_once("currently displaying: image:") else {
            continue;
        };
        let path = path.trim();
        if path.is_empty() {
            continue;
        }
        let output = head
            .split(':')
            .map(str::trim)
            .find(|s| !s.is_empty())
            .map(str::to_string);
        result.push(MonitorWallpaper {
            output,
            path: PathBuf::from(path),
        });
    }

    if result.is_empty() {
        return Err(format!("could not parse wallpaper from:\n{stdout}"));
    }

    for m in &mut result {
        let expanded = expand_tilde(&m.path.to_string_lossy());
        let p = Path::new(&expanded);
        if !p.is_file() {
            return Err(format!("wallpaper does not exist: {expanded}"));
        }
        m.path = p.to_path_buf();
    }
    Ok(result)
}

/// First occurrence of every distinct wallpaper path, order preserved.
pub fn unique_by_path(wallpapers: &[MonitorWallpaper]) -> Vec<MonitorWallpaper> {
    let mut seen = HashSet::new();
    wallpapers
        .iter()
        .filter(|m| seen.insert(m.path.clone()))
        .cloned()
        .collect()
}

fn expand_tilde(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~/")
        && let Some(home) = std::env::var_os("HOME")
    {
        return format!("{}/{}", home.to_string_lossy(), rest);
    }
    path.to_string()
}
