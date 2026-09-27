# hyprlock-wrapper

Rust binary `hyprlock-accent` (`src/`, `cargo build --release`) computes the
accent color + horizontal clock offset for hyprlock from the current `awww`
wallpaper.

## Implementation (hyprlock-accent)
- Cache `~/.cache/hyprlock-accent.json`: {path_md5: {wallpaper, accent, foreground, y_offset, src_mtime, src_size, max_width, column_frac}}. `src_mtime`/`src_size` (serde default) invalidate by content: replacing the file at the same path triggers recomputation (the pinned offset is then lost — by design). `max_width`/`column_frac` — cache hit only under the same analysis parameters (checked in `main`, NOT in `load_cached`, so the pin is not lost). Cache writes are atomic (tmp + rename); the file stamp comes from a single `stat`.
- An entry with empty accent/foreground (after `--set-offset` with no computation) is treated as incomplete: colors are recomputed, the pinned `y_offset` is kept.
- Porting gotchas (matter when editing): `edge_profile` returns `w-1` columns, `saliency_map` returns `w`. `np.convolve(...,'same')` = centered window always dividing by K (not trailing/normalized). `fast_image_resize` LANCZOS differs by ±1 in the low bit vs PIL on edge pixels → colors off by ±1, y_offset matches exactly.
- Multi-output (implemented): `awww query` is parsed into (output, path) — line format `: HDMI-A-1: ... currently displaying: image: /path`. One unique path → legacy env mode (user config untouched, zero risk). ≥2 paths → generation: `src/config.rs` takes `~/.config/hypr/hyprlock.conf` as a template, substitutes $WALLPAPER/$accent/$foreground/$y_offset with literals and inserts `monitor = <output>` into every widget block (an existing `monitor=` is replaced), concatenates the renders and writes `/tmp/hyprlock-accent-$USER.conf`, launching `hyprlock -c`. `--print-config` prints the config instead of launching. Fallbacks: output names unparseable or no template → single mode with a warning. `--set-offset` in multi mode → error, asks for HYPRLOCK_WALLPAPER. HYPRLOCK_ACCENT/FOREGROUND env overrides apply to all monitors. NOT verified against a live lock: behavior of a duplicated input-field on two monitors (hyprlock wiki is silent, the password buffer is session state).
- Rust API: `fast_image_resize::images::Image` (not re-exported at the crate root); resize → `dst.into_vec()` (no `write_back`); `ResizeAlg::Convolution(FilterType::Lanczos3)`.

## Key decisions
- `foreground` — single color, format `RRGGBBDD` (NO leading `#`, alpha DD = 221 ≈0.867). In config: `rgba($foreground)` for gradient fields, `#$foreground` in Pango spans.
- hyprlock color field types DIFFER: `color`/`font_color`/`inner_color` — type `color` (accept `#hex` or `rgba(r,g,b,a)`); `outer_color` (input-field) and `border_color` (shape/image) — type `gradient` (only `rgba(...)`, no hex, alpha inside `rgba`; a single color = `rgba(RRGGBBAA)`). Hence an 8-digit `#RRGGBBAA` on a gradient field is silently ignored.
- Pango markup inside `text = cmd[...] echo ...`: in the CONFIG a double `#` (`##`) = literal `#` after hyprlock parses (the first `#` is the escape/non-comment). For a color variable without `#` write `##$foreground`, so the shell receives `#$foreground` → Pango sees `#RRGGBBDD`. Bare `#$foreground` breaks: `#` in config = comment, cuts the line → shell `unexpected EOF`.
- `y_offset` — signed percentage of screen width (`-N%` left, `+N%` right, `0` center). Printed with the percent sign, because the config expects `position = $y_offset, 0`.
- Occupancy metric (v2): **saliency_map** = normalized sum of two per-column profiles — (a) silhouette edges (max horizontal gradient) and (b) brightness (mean luminance). The combination is needed: edge-only misses glowing/smooth objects, brightness-only smears mass. Then: `_occupied_segments` at threshold 0.35 → free gaps between objects and edges → column centered in the widest gap (≥28% of width). Fallback for dense wallpapers (no gap): sliding min edge-only + edge_penalty 0.1*(dist−0.15).
- Searches for a vertical dark column (shape) ~28% of screen width: saliency object segmentation + center of the free gap; fallback for dense images.
- Manual override: `--set-offset N` writes `~/.cache/hyprlock-accent.json` (keyed by wallpaper md5); the tool then uses it instead of the computed value.

## Usage
```
hyprlock-accent               # compute and launch hyprlock
hyprlock-accent --no-launch   # only print WALLPAPER/foreground/y_offset
hyprlock-accent --set-offset -25  # pin manually
hyprlock-accent -- -g 2       # hyprlock flags after --
```

## Environment
- Build: `cargo build --release`; install `install -Dm755 target/release/hyprlock-accent ~/.local/bin/`.
- Checks: `cargo check` / `cargo clippy`.
- Hygiene: `y_offset` must be printed with `%`; color is 8 hex chars.
- Test wallpapers are full 4K (3840x2160); analysis at 800w downscale.
- Packaging: `PKGBUILD` in the repo (`hyprlock-accent-git`, MIT, cargo build). See skill `pkgbuild`.

## Packaging (makepkg) — important
- Do NOT run `makepkg` in the project root: build in a separate folder `mkdir /tmp/pk && cp PKGBUILD /tmp/pk/ && cd /tmp/pk && makepkg -si`.
- PKGBUILD (fixed): `build()`/`package()` do `cd "$_pkgbase"` (the git-source checkout lives in `$srcdir/$_pkgbase`; without the cd, cargo/LICENSE are not found); `cargo fetch --locked` before `cargo build --frozen` (frozen = offline + exact lock; fetch refreshes the local sparse index without touching Cargo.lock — otherwise on machines with a stale cache, serde-style versions fail to resolve); LICENSE is installed from `src/LICENSE` (deliberately located there).
- `RUSTUP_TOOLCHAIN=stable` in PKGBUILD is required: on machines without a default toolchain, bare `cargo` fails.

## Git files & memory
- README.md (single, root, in English): tracked. `src/hyprlock-accent/` is a makepkg checkout (repo clone), NOT in git (`.gitignore`): it holds a copy of README.md. Do NOT treat `src/hyprlock-accent/` as the source of truth — edit docs only in the root README.md.
- Session memory: `.opencode/NOTES.md`.

## README: discrepancies with the code (checked in a live session)
- ✅ Updated and translated to English (2026-09-28): added `--print-config`, multi-monitor mode, `-h/--help`; fixed "foreground = brightest" (no "non-white"), cache invalidation (content + max_width/column_frac), fallback (least edge energy, not "centers in the gap"). The section below is kept as historical reference.
- (historical) README did not document `--print-config` and multi-monitor — added during translation.
- (historical) "foreground = brightest (non-white)" — there is NO non-white filter in the code (`bright_color` = mean top-0.1% by luminance, can yield white).
- (historical) "recompute only on wallpaper change" — actually also on content change of the same path (src_mtime/src_size) and on max_width/column_frac change.
- (historical) fallback "centers in the gap" when no gap exists — actually picks the position of minimum edge energy (`edge_profile` + penalty).
