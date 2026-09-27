# hyprlock-accent

Compute the accent color, foreground color, and horizontal clock offset for
[hyprlock](https://github.com/hyprwm/hyprlock) from the current `awww`
wallpaper.

Written in Rust. Cold start (decode + analysis) is ~110 ms; with a warm cache
it drops to ~3-4 ms.

## How it works

1. Reads the current wallpaper path(s) from `awww query`.
2. Segments the wallpaper into objects (saliency = edges + brightness) and
   places a vertical clock column (~28% of the screen width) in the widest
   object-free gap. `y_offset` is a signed percentage (0 = center, negative =
   left, positive = right). For dense images with no fitting gap it falls back
   to the position of least edge energy.
3. Accent = the most vivid (bright + saturated) color, foreground = the
   brightest color. Both are `RRGGBBFF` with no leading `#`.
4. Exports `WALLPAPER` / `accent` / `foreground` / `y_offset` and launches
   `hyprlock`.

Results are cached in `~/.cache/hyprlock-accent.json`, keyed by the md5 of the
resolved wallpaper path. Recomputation happens when the wallpaper changes, when
the file contents change (same path, new image), or when `--max-width` /
`--column-frac` change.

## Installation

```sh
cargo build --release
install -Dm755 target/release/hyprlock-accent ~/.local/bin/hyprlock-accent
```

Arch Linux: a `PKGBUILD` is provided (package `hyprlock-accent-git`). Build it
outside the project directory — not in the repo root:

```sh
mkdir /tmp/pkg && cp PKGBUILD /tmp/pkg/ && cd /tmp/pkg && makepkg -si
```

## Usage

```sh
hyprlock-accent                     # compute and launch hyprlock
hyprlock-accent --no-launch         # print values only
hyprlock-accent --print-config      # print the rendered per-monitor config
hyprlock-accent --set-offset -25    # pin y_offset manually, then exit
hyprlock-accent --max-width 1200 --column-frac 0.3
hyprlock-accent -- -g 2             # args after -- are passed to hyprlock
```

| Flag | Description |
| --- | --- |
| `--no-launch` | Print computed values and exit. |
| `--print-config` | Print the rendered per-monitor config and exit. |
| `--max-width <px>` | Downscale width for analysis (default: 800). |
| `--column-frac <f>` | Clock column width as a fraction of the screen, `0..1` (default: `0.28`). |
| `--set-offset <pct>` | Pin `y_offset` for the wallpaper and exit. |
| `-h`, `--help` | Show help. |

Arguments after `--` are forwarded to `hyprlock` unchanged.

## Environment variables

| Variable | Description |
| --- | --- |
| `HYPRLOCK_WALLPAPER` | Force a wallpaper path (skips `awww query`). |
| `HYPRLOCK_ACCENT` | Override the accent color (`RRGGBB` or `RRGGBBAA`, no `#`). |
| `HYPRLOCK_FOREGROUND` | Override the foreground color (`RRGGBB` or `RRGGBBAA`, no `#`). |

## hyprlock integration

The tool reads `awww query` per output.

- **Single wallpaper** (one distinct path, or a single forced path): values are
  passed as env vars and your stock `hyprlock.conf` is left untouched. Reference
  them as `$accent`, `$foreground`, `$y_offset`, `$WALLPAPER`.
- **Multiple wallpapers** (distinct image per output): your
  `~/.config/hypr/hyprlock.conf` is used as a template. Its `$WALLPAPER`,
  `$accent`, `$foreground`, `$y_offset` placeholders are substituted with
  literal values and each widget block gets `monitor = <output>`. The result is
  written to `/tmp/hyprlock-accent-$USER.conf` and launched with
  `hyprlock -c <config>`. Use `--print-config` to inspect the rendered config.

If output names cannot be parsed or the template is missing, it falls back to
the first wallpaper for all monitors with a warning.

## Color format notes

- `foreground` / `accent` are 8 hex digits (`RRGGBBAA`), **no** leading `#`.
- hyprlock has two color field types: `color` (accepts `#hex` or
  `rgba(r,g,b,a)`) and `gradient` (`rgba(...)` only, alpha inside; a single
  color is `rgba(RRGGBBAA)`). An 8-digit `#RRGGBBAA` on a `gradient` field is
  silently ignored.
- In Pango markup inside `text = cmd[...] echo ...` use a double `#` (`##$acc`):
  the first `#` is the config escape, so `##` becomes a literal `#` after
  hyprlock parses the line.

## License

MIT.
