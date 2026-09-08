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

fn print_help() {
    println!(
        "usage: hyprlock-accent [options] [-- hyprlock-args]\n\
         \n\
         options:\n\
         \x20 --no-launch            print computed values and exit\n\
         \x20 --max-width <px>       downscale width for analysis (default: 800)\n\
         \x20 --column-frac <f>      clock column width as width fraction, 0..1 (default: 0.28)\n\
         \x20 --set-offset <pct>     pin y_offset percentage for the wallpaper and exit\n\
         \x20 -h, --help             show this help\n\
         \n\
         env:\n\
         \x20 HYPRLOCK_WALLPAPER     force wallpaper path\n\
         \x20 HYPRLOCK_ACCENT        override accent color (RRGGBB[AA])\n\
         \x20 HYPRLOCK_FOREGROUND    override foreground color (RRGGBB[AA])\n\
         \n\
         arguments after -- are passed to hyprlock."
    );
}

/// Value for `--opt <v>` / `--opt=<v>`. A following `--`-prefixed argument is
/// never consumed as a value (reported as missing by the caller instead).
fn opt_value(
    flag: &str,
    arg: &str,
    args: &mut std::iter::Peekable<impl Iterator<Item = String>>,
) -> Option<String> {
    if let Some(v) = arg.strip_prefix(&format!("{flag}=")) {
        return Some(v.to_string());
    }
    let next_is_flag = args.peek().is_some_and(|v| v.starts_with("--"));
    if next_is_flag { None } else { args.next() }
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
            "-h" | "--help" => {
                print_help();
                std::process::exit(0);
            }
            "--no-launch" => no_launch = true,
            a if a == "--max-width" || a.starts_with("--max-width=") => {
                match opt_value("--max-width", a, &mut args)
                    .and_then(|s| s.parse::<u32>().ok())
                    .filter(|&v| v > 0)
                {
                    Some(v) => max_width = v,
                    None => eprintln!(
                        "warning: --max-width needs a positive integer; using {max_width}"
                    ),
                }
            }
            a if a == "--column-frac" || a.starts_with("--column-frac=") => {
                match opt_value("--column-frac", a, &mut args)
                    .and_then(|s| s.parse::<f64>().ok())
                    .filter(|&v| v > 0.0 && v <= 1.0)
                {
                    Some(v) => column_frac = v,
                    None => eprintln!(
                        "warning: --column-frac needs a number in (0, 1]; using {column_frac}"
                    ),
                }
            }
            a if a == "--set-offset" || a.starts_with("--set-offset=") => {
                match opt_value("--set-offset", a, &mut args).and_then(|s| s.parse::<i32>().ok())
                {
                    Some(v) => set_offset = Some(v),
                    None => {
                        eprintln!("warning: --set-offset needs an integer percentage; ignoring")
                    }
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
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            eprintln!("error: hyprlock not found in PATH");
            127
        }
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

    let entry = CachedEntry::new(
        wallpaper.to_string(),
        accent.clone(),
        foreground.clone(),
        offset,
    );
    cache::store(wallpaper, entry, (max_width, column_frac));

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

    // cached full result (per wallpaper) or fresh compute. Hits require the
    // same analysis parameters the entry was computed with; pin-only or legacy
    // entries count as misses and the pinned y_offset is preserved by
    // compute_and_store().
    let (accent_base, foreground_base, y_offset) = match cache::load_cached(&wallpaper) {
        Some(e) if e.matches_analysis(args.max_width, args.column_frac) => {
            (e.accent, e.foreground, e.y_offset)
        }
        _ => match compute_and_store(&wallpaper, args.max_width, args.column_frac) {
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
