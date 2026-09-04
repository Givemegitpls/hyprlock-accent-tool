//! Compute wallpaper accent/foreground colors + clock y_offset, then launch
//! hyprlock. Rust rewrite of `hyprlock_accent.py`.

mod analysis;
mod cache;
mod wallpaper;

use cache::CachedEntry;
use std::process::Command;

const ALPHA: &str = "FF"; // 255 = 1

struct Args {
    no_launch: bool,
    max_width: u32,
    column_frac: f64,
    set_offset: Option<i32>,
    hyprlock_args: Vec<String>,
}

fn parse_args() -> Args {
    let mut args = std::env::args().skip(1).peekable();
    let mut no_launch = false;
    let mut max_width = 800u32;
    let mut column_frac = 0.28f64;
    let mut set_offset: Option<i32> = None;
    let mut hyprlock_args: Vec<String> = Vec::new();
    let mut after_dashdash = false;

    while let Some(arg) = args.next() {
        if after_dashdash {
            hyprlock_args.push(arg);
            continue;
        }
        match arg.as_str() {
            "--" => after_dashdash = true,
            "--no-launch" => no_launch = true,
            "--max-width" => {
                if let Some(v) = args.next() {
                    max_width = v.parse().unwrap_or(800);
                }
            }
            "--column-frac" => {
                if let Some(v) = args.next() {
                    column_frac = v.parse().unwrap_or(0.28);
                }
            }
            "--set-offset" => {
                if let Some(v) = args.next() {
                    set_offset = v.parse().ok();
                }
            }
            other => hyprlock_args.push(other.to_string()),
        }
    }

    Args {
        no_launch,
        max_width,
        column_frac,
        set_offset,
        hyprlock_args,
    }
}

/// Env-provided color override (`RRGGBB` or `RRGGBBAA`, no leading `#`), else
/// the computed default. 6-digit values gain the default `FF` alpha.
fn override_color(var_name: &str, default: &str) -> String {
    let Some(raw) = std::env::var(var_name).ok() else {
        return default.to_string();
    };
    if raw.is_empty() {
        return default.to_string();
    }
    let value = raw.trim().trim_start_matches('#');
    let is_hex = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_hexdigit());
    if value.len() == 6 && is_hex(value) {
        format!("{}{}", value.to_uppercase(), ALPHA)
    } else if value.len() == 8 && is_hex(value) {
        value.to_uppercase()
    } else {
        eprintln!("warning: {var_name}={raw:?} not RRGGBB[AA]; ignoring");
        default.to_string()
    }
}

fn run_hyprlock(env: &[(String, String)], extra_args: &[String]) -> ! {
    let mut cmd = Command::new("hyprlock");
    cmd.args(extra_args);
    for (k, v) in env {
        cmd.env(k, v);
    }
    let status = cmd.status();
    let code = match status {
        Ok(s) => s.code().unwrap_or(1),
        Err(e) => {
            eprintln!("error: failed to launch hyprlock: {e}");
            1
        }
    };
    std::process::exit(code);
}

fn compute_and_store(
    wallpaper: &str,
    max_width: u32,
    column_frac: f64,
) -> Result<(String, String, i32), String> {
    let (rgb, w, h) =
        analysis::load_image(wallpaper, max_width).map_err(|e| format!("error: {e}"))?;

    let computed = analysis::compute_y_offset(&rgb, w as usize, h as usize, column_frac, 0.35);
    let offset = cache::cached_offset(wallpaper, computed);
    let accent = analysis::vivid_color(&rgb, w as usize, h as usize, 0.005);
    let foreground = analysis::bright_color(&rgb, w as usize, h as usize, 0.001);

    let entry = CachedEntry {
        wallpaper: wallpaper.to_string(),
        accent: accent.clone(),
        foreground: foreground.clone(),
        y_offset: offset,
    };
    cache::store(wallpaper, entry);

    Ok((accent, foreground, offset))
}

fn main() {
    let args = parse_args();

    let wallpaper = match wallpaper::get_wallpaper() {
        Ok(w) => w.to_string_lossy().into_owned(),
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    };

    // --set-offset N: persist and exit (no launch, no compute)
    if let Some(value) = args.set_offset {
        cache::pin_offset(&wallpaper, value);
        println!("pinned y_offset={value}% for {wallpaper}");
        return;
    }

    // cached full result (per wallpaper) or fresh compute
    let (accent_base, foreground_base, y_offset) = match cache::load_cached(&wallpaper) {
        Some(e) => (e.accent, e.foreground, e.y_offset),
        None => match compute_and_store(&wallpaper, args.max_width, args.column_frac) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(1);
            }
        },
    };

    let accent = override_color("HYPRLOCK_ACCENT", &format!("{accent_base}{ALPHA}"));
    let foreground = override_color("HYPRLOCK_FOREGROUND", &format!("{foreground_base}{ALPHA}"));

    if args.no_launch {
        println!("WALLPAPER={wallpaper}");
        println!("accent={accent}");
        println!("foreground={foreground}");
        println!("y_offset={y_offset}%");
        return;
    }

    let env = vec![
        ("WALLPAPER".to_string(), wallpaper.clone()),
        ("accent".to_string(), accent),
        ("foreground".to_string(), foreground),
        ("y_offset".to_string(), format!("{y_offset}%")),
    ];

    run_hyprlock(&env, &args.hyprlock_args);
}
