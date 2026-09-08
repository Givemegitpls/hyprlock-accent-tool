//! Render the user's hyprlock config with per-monitor literal values.
//!
//! The template is the user's own `hypr/hyprlock.conf`. Its `$WALLPAPER`,
//! `$accent`, `$foreground` and `$y_offset` placeholders are substituted with
//! concrete values, and every widget block gets `monitor = <output>` (an
//! existing `monitor` line is replaced), so each output renders its own copy
//! of the whole layout.

use std::path::PathBuf;

const WIDGETS: [&str; 5] = ["background", "image", "label", "input-field", "shape"];

/// Values for one monitor's render pass.
pub struct RenderValues {
    pub output: String,
    pub wallpaper: String,
    /// Full `RRGGBBAA`, same as the `accent` env var carried before.
    pub accent: String,
    pub foreground: String,
    /// Already suffixed with `%` (e.g. `-28%`).
    pub y_offset: String,
}

/// The user's stock hyprlock config, used as the render template.
pub fn template_path() -> Option<PathBuf> {
    let base = match std::env::var_os("XDG_CONFIG_HOME") {
        Some(d) => PathBuf::from(d),
        None => PathBuf::from(std::env::var_os("HOME")?).join(".config"),
    };
    let p = base.join("hypr").join("hyprlock.conf");
    p.is_file().then_some(p)
}

/// Render the template once per entry and concatenate.
pub fn render(template: &str, values: &[RenderValues]) -> String {
    values
        .iter()
        .map(|v| render_one(template, v))
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_one(template: &str, v: &RenderValues) -> String {
    let mut out = String::new();
    let mut in_widget = false;
    let mut monitor_done = false;

    for line in template.lines() {
        let substituted = substitute(line, v);
        let trimmed = substituted.trim_start();

        if !in_widget && is_widget_open(trimmed) {
            in_widget = true;
            monitor_done = false;
            out.push_str(&substituted);
            out.push('\n');
            continue;
        }

        if in_widget && trimmed.starts_with('}') {
            if !monitor_done {
                out.push_str(&format!("    monitor = {}\n", v.output));
            }
            in_widget = false;
            out.push_str(&substituted);
            out.push('\n');
            continue;
        }

        if in_widget && trimmed.starts_with("monitor") && trimmed.contains('=') {
            out.push_str(&format!("    monitor = {}\n", v.output));
            monitor_done = true;
            continue;
        }

        out.push_str(&substituted);
        out.push('\n');
    }
    out
}

fn substitute(line: &str, v: &RenderValues) -> String {
    line.replace("$WALLPAPER", &v.wallpaper)
        .replace("$accent", &v.accent)
        .replace("$foreground", &v.foreground)
        .replace("$y_offset", &v.y_offset)
}

fn is_widget_open(trimmed: &str) -> bool {
    WIDGETS
        .iter()
        .any(|w| trimmed.strip_prefix(w).is_some_and(|rest| rest.trim_start().starts_with('{')))
}
