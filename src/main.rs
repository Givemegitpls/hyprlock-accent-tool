//! Compute wallpaper accent/foreground colors + clock y_offset, then launch
//! hyprlock. Rust rewrite of `hyprlock_accent.py`.

mod analysis;
mod cache;
mod config;
mod wallpaper;

use cache::CachedEntry;
use std::process::Command;

const ALPHA: &str = "FF"; // 255 = 1

struct Args {
    no_launch: bool,
    print_config: bool,
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
         \x20 --print-config         print the rendered per-monitor config and exit\n\
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
    let mut print_config = false;
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
            "--print-config" => print_config = true,
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
        print_config,
        max_width,
        column_frac,
        set_offset,
        hyprlock_args,
    }
}

/// Validated `HYPRLOCK_*` color override (`RRGGBB` or `RRGGBBAA`, no `#`),
/// else `None`. 6-digit values gain the default `FF` alpha.
fn env_override(var_name: &str) -> Option<String> {
    let raw = std::env::var(var_name).ok()?;
    let value = raw.trim().trim_start_matches('#');
    let is_hex = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_hexdigit());
    if value.len() == 6 && is_hex(value) {
        Some(format!("{}{ALPHA}", value.to_uppercase()))
    } else if value.len() == 8 && is_hex(value) {
        Some(value.to_uppercase())
    } else {
        eprintln!("warning: {var_name}={raw:?} not RRGGBB[AA]; ignoring");
        None
    }
}

fn run_hyprlock(config: Option<&str>, env: &[(String, String)], extra_args: &[String]) -> ! {
    let mut cmd = Command::new("hyprlock");
    if let Some(c) = config {
        cmd.arg("-c").arg(c);
    }
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

/// Per-wallpaper resolved values, colors already include alpha.
struct Resolved {
    wallpaper: wallpaper::MonitorWallpaper,
    accent: String,
    foreground: String,
    y_offset: i32,
}

fn main() {
    let args = parse_args();

    let wallpapers = match wallpaper::get_wallpapers() {
        Ok(w) => w,
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    };
    let unique = wallpaper::unique_by_path(&wallpapers);

    // --set-offset N: persist and exit (no launch, no compute). Needs an
    // unambiguous target wallpaper.
    if let Some(value) = args.set_offset {
        if unique.len() > 1 {
            let names = unique
                .iter()
                .map(|m| {
                    m.output
                        .clone()
                        .unwrap_or_else(|| m.path.to_string_lossy().into_owned())
                })
                .collect::<Vec<_>>()
                .join(", ");
            eprintln!("error: multiple wallpapers active ({names}); set HYPRLOCK_WALLPAPER to pin one");
            std::process::exit(1);
        }
        let path = unique[0].path.to_string_lossy().into_owned();
        cache::pin_offset(&path, value);
        println!("pinned y_offset={value}% for {path}");
        return;
    }

    let accent_override = env_override("HYPRLOCK_ACCENT");
    let foreground_override = env_override("HYPRLOCK_FOREGROUND");

    // cached full result (per wallpaper) or fresh compute. Hits require the
    // same analysis parameters the entry was computed with; pin-only or legacy
    // entries count as misses and the pinned y_offset is preserved by
    // compute_and_store().
    let mut resolved: Vec<Resolved> = Vec::new();
    for m in &unique {
        let wp = m.path.to_string_lossy().into_owned();
        let (accent_base, foreground_base, y_offset) = match cache::load_cached(&wp) {
            Some(e) if e.matches_analysis(args.max_width, args.column_frac) => {
                (e.accent, e.foreground, e.y_offset)
            }
            _ => match compute_and_store(&wp, args.max_width, args.column_frac) {
                Ok(t) => t,
                Err(e) => {
                    eprintln!("{e}");
                    std::process::exit(1);
                }
            },
        };
        resolved.push(Resolved {
            wallpaper: m.clone(),
            accent: accent_override
                .clone()
                .unwrap_or_else(|| format!("{accent_base}{ALPHA}")),
            foreground: foreground_override
                .clone()
                .unwrap_or_else(|| format!("{foreground_base}{ALPHA}")),
            y_offset,
        });
    }

    if resolved.len() == 1 {
        run_single(&args, resolved.remove(0));
    } else {
        run_multi(&args, resolved);
    }
}

/// One wallpaper for everything: values via env vars, stock config untouched.
fn run_single(args: &Args, r: Resolved) {
    let wallpaper = r.wallpaper.path.to_string_lossy().into_owned();
    if args.print_config {
        eprintln!("note: single-wallpaper mode feeds values via env vars, nothing to render");
    }
    if args.no_launch || args.print_config {
        println!("WALLPAPER={wallpaper}");
        println!("accent={}", r.accent);
        println!("foreground={}", r.foreground);
        println!("y_offset={}%", r.y_offset);
        return;
    }

    let env = vec![
        ("WALLPAPER".to_string(), wallpaper.clone()),
        ("accent".to_string(), r.accent),
        ("foreground".to_string(), r.foreground),
        ("y_offset".to_string(), format!("{}%", r.y_offset)),
    ];
    run_hyprlock(None, &env, &args.hyprlock_args);
}

/// Distinct wallpaper per output: render the user's template once per monitor
/// with `monitor = <output>` and literal values, launch hyprlock with it.
fn run_multi(args: &Args, mut resolved: Vec<Resolved>) {
    if resolved.iter().any(|r| r.wallpaper.output.is_none()) {
        eprintln!(
            "warning: awww output names unavailable; falling back to the first wallpaper for all monitors"
        );
        run_single(args, resolved.remove(0));
        return;
    }

    let template_path = match config::template_path() {
        Some(p) => p,
        None => {
            eprintln!(
                "error: cannot render per-monitor config: hypr/hyprlock.conf not found under XDG_CONFIG_HOME or ~/.config"
            );
            std::process::exit(1);
        }
    };
    let template = match std::fs::read_to_string(&template_path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("error: cannot read {}: {e}", template_path.display());
            std::process::exit(1);
        }
    };

    let values: Vec<config::RenderValues> = resolved
        .iter()
        .map(|r| config::RenderValues {
            output: r.wallpaper.output.clone().unwrap_or_default(),
            wallpaper: r.wallpaper.path.to_string_lossy().into_owned(),
            accent: r.accent.clone(),
            foreground: r.foreground.clone(),
            y_offset: format!("{}%", r.y_offset),
        })
        .collect();
    let rendered = config::render(&template, &values);

    if args.print_config {
        print!("{rendered}");
        return;
    }

    if args.no_launch {
        for r in &resolved {
            println!(
                "[{}] WALLPAPER={} accent={} foreground={} y_offset={}%",
                r.wallpaper.output.clone().unwrap_or_default(),
                r.wallpaper.path.display(),
                r.accent,
                r.foreground,
                r.y_offset
            );
        }
        return;
    }

    let config_path = match std::env::var("USER") {
        Ok(u) => format!("/tmp/hyprlock-accent-{u}.conf"),
        Err(_) => "/tmp/hyprlock-accent.conf".to_string(),
    };
    if let Err(e) = std::fs::write(&config_path, &rendered) {
        eprintln!("error: cannot write {config_path}: {e}");
        std::process::exit(1);
    }
    run_hyprlock(Some(&config_path), &[], &args.hyprlock_args);
}
