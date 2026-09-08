//! Wallpaper acquisition via `awww query`.

use std::path::PathBuf;
use std::process::Command;

/// Return the current awww wallpaper path.
///
/// `HYPRLOCK_WALLPAPER` overrides the query (testing / fixed wallpapers).
pub fn get_wallpaper() -> Result<PathBuf, String> {
    if let Some(forced) = std::env::var_os("HYPRLOCK_WALLPAPER") {
        let p = std::path::Path::new(&forced);
        if !p.is_file() {
            return Err(format!("wallpaper does not exist: {}", p.display()));
        }
        return Ok(p.to_path_buf());
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

    let paths: Vec<&str> = stdout
        .lines()
        .filter_map(|line| line.split("image:").nth(1))
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();

    // multi-monitor: awww reports one image per output; dedupe, and warn when
    // outputs disagree (we cannot know which output hosts the clock column).
    let mut unique = paths.clone();
    unique.sort_unstable();
    unique.dedup();
    let path = match unique.as_slice() {
        [] => return Err(format!("could not parse wallpaper from:\n{stdout}")),
        [only] => (*only).to_string(),
        [first, ..] => {
            eprintln!(
                "warning: multiple wallpapers active ({} outputs); using {first}",
                paths.len()
            );
            (*first).to_string()
        }
    };

    let path = expand_tilde(&path);
    let p = std::path::Path::new(&path);
    if !p.is_file() {
        return Err(format!("wallpaper does not exist: {path}"));
    }
    Ok(p.to_path_buf())
}

fn expand_tilde(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~/")
        && let Some(home) = std::env::var_os("HOME")
    {
        return format!("{}/{}", home.to_string_lossy(), rest);
    }
    path.to_string()
}
