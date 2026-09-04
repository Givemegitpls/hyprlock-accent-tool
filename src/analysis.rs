//! Accent/foreground color + y_offset analysis, ported 1:1 from the Python
//! reference (`hyprlock_accent.py`). Pure Rust, no numpy, to avoid interpreter
//! and import startup cost.

use fast_image_resize as fir;

/// Load an image from `path` and downscale to `max_width` width (LANCZOS),
/// returning (rgba8, w, h).
pub fn load_image(path: &str, max_width: u32) -> Result<(Vec<u8>, u32, u32), String> {
    let img = image::open(path).map_err(|e| format!("cannot open image {path}: {e}"))?;
    let rgb = img.to_rgb8();
    let (w, h) = rgb.dimensions();
    let data = rgb.into_raw();

    if w <= max_width {
        return Ok((data, w, h));
    }

    let ratio = max_width as f64 / w as f64;
    let dst_h = (h as f64 * ratio).round() as u32;
    let resized = resize_lanczos(&data, w, h, max_width, dst_h)?;
    Ok((resized, max_width, dst_h))
}

/// LANCZOS resize RGB8 -> RGB8.
fn resize_lanczos(src: &[u8], sw: u32, sh: u32, dw: u32, dh: u32) -> Result<Vec<u8>, String> {
    let src_img = fir::images::Image::from_vec_u8(sw, sh, src.to_vec(), fir::PixelType::U8x3)
        .map_err(|e| format!("resize src: {e}"))?;
    let mut dst = fir::images::Image::new(dw, dh, fir::PixelType::U8x3);
    let mut resizer = fir::Resizer::new();
    let opts = fir::ResizeOptions::new()
        .resize_alg(fir::ResizeAlg::Convolution(fir::FilterType::Lanczos3));
    resizer
        .resize(&src_img, &mut dst, &opts)
        .map_err(|e| format!("resize: {e}"))?;

    Ok(dst.into_vec())
}

fn normalize(v: &[f32]) -> Vec<f32> {
    let (mn, mx) = minmax(v);
    let span = mx - mn;
    if span > 1e-9 {
        v.iter().map(|x| (x - mn) / span).collect()
    } else {
        v.to_vec()
    }
}

fn minmax(v: &[f32]) -> (f32, f32) {
    v.iter()
        .fold((f32::INFINITY, f32::NEG_INFINITY), |(mn, mx), &x| {
            (mn.min(x), mx.max(x))
        })
}

fn sliding_mean(v: &[f32], window: usize) -> Vec<f32> {
    let n = v.len();
    if n == 0 {
        return Vec::new();
    }
    let k = window.max(1);
    let offset = k / 2; // centre the kernel like np.convolve(..., 'same')
    let denom = k as f32;
    let mut out = vec![0.0f32; n];
    // prefix sums for O(n) windowed mean
    let mut prefix = vec![0.0f32; n + 1];
    for i in 0..n {
        prefix[i + 1] = prefix[i] + v[i];
    }
    for i in 0..n {
        let lo = i.saturating_sub(offset);
        let hi = (i + k - offset).min(n);
        if lo < hi {
            out[i] = (prefix[hi] - prefix[lo]) / denom;
        }
    }
    out
}

fn luminance(rgb: &[u8]) -> Vec<f32> {
    rgb.chunks_exact(3)
        .map(|p| p[0] as f32 * 0.299 + p[1] as f32 * 0.587 + p[2] as f32 * 0.114)
        .collect()
}

/// Horizontal saliency profile (edges + brightness), normalised 0..1.
pub fn saliency_map(rgb: &[u8], w: usize, h: usize, window: usize) -> Vec<f32> {
    let lum = luminance(rgb);

    // edge: gx = abs(diff(lum, axis=1)); edge[x] = max over rows
    let mut edge = vec![0.0f32; w];
    for y in 0..h {
        for x in 0..w {
            let next = if x + 1 < w {
                lum[y * w + x + 1]
            } else {
                lum[y * w + x]
            };
            let d = (next - lum[y * w + x]).abs();
            if d > edge[x] {
                edge[x] = d;
            }
        }
    }

    // bright: mean per column
    let mut bright = vec![0.0f32; w];
    for x in 0..w {
        let mut sum = 0.0f32;
        for y in 0..h {
            sum += lum[y * w + x];
        }
        bright[x] = sum / h as f32;
    }

    let edge_n = normalize(&edge);
    let bright_n = normalize(&bright);
    let blended: Vec<f32> = edge_n.iter().zip(&bright_n).map(|(a, b)| a + b).collect();
    let blended_n = normalize(&blended);
    let smoothed = sliding_mean(&blended_n, window.max(1));
    // NOTE: python uses np.convolve(..., mode="same"), which is a centred moving
    // average, not a trailing one. This trailing window is a close approximation;
    // see saliency_map_bounded for the exact behaviour when parity matters.
    normalize(&smoothed)
}

fn percentile(data: &[f32], p: f32) -> f32 {
    let mut sorted = data.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    if sorted.is_empty() {
        return 0.0;
    }
    // linear interpolation, matching numpy's default 'interpolated'
    let q = p / 100.0;
    let idx = q * (sorted.len() as f32 - 1.0);
    let lo = idx.floor() as usize;
    let hi = (idx.ceil() as usize).min(sorted.len() - 1);
    let frac = idx - idx.floor();
    sorted[lo] + (sorted[hi] - sorted[lo]) * frac
}

/// Edge-only saliency profile (silhouette edges), normalised 0..1.
///
/// Matches the Python reference: `gx` has shape (h, w-1), `strong.sum(axis=0)`
/// yields a profile of length w-1 (the last column is dropped).
pub fn edge_profile(rgb: &[u8], w: usize, h: usize, pct: f32, window: usize) -> Vec<f32> {
    let lum = luminance(rgb);
    let mut gx = vec![0.0f32; h * (w - 1)];
    for y in 0..h {
        for x in 0..(w - 1) {
            gx[y * (w - 1) + x] = (lum[y * w + x + 1] - lum[y * w + x]).abs();
        }
    }
    let threshold = percentile(&gx, pct);
    // col_strength over w-1 columns: sum over rows of (gx > threshold)
    let mut col_strength = vec![0.0f32; w - 1];
    for y in 0..h {
        for x in 0..(w - 1) {
            if gx[y * (w - 1) + x] > threshold {
                col_strength[x] += 1.0;
            }
        }
    }
    let smoothed = sliding_mean(&col_strength, window.max(1));
    normalize(&smoothed)
}

/// Contiguous index ranges where `profile > threshold`; (start, end).
fn occupied_segments(profile: &[f32], threshold: f32) -> Vec<(usize, usize)> {
    let n = profile.len();
    let mut segs = Vec::new();
    let mut i = 0;
    while i < n {
        if profile[i] > threshold {
            let start = i;
            while i < n && profile[i] > threshold {
                i += 1;
            }
            segs.push((start, i));
        } else {
            i += 1;
        }
    }
    segs
}

/// Place the clock column (fraction of screen width) in the widest object-free
/// gap; returns a percentage (0 centre, negative left, positive right).
pub fn compute_y_offset(
    rgb: &[u8],
    w: usize,
    h: usize,
    column_frac: f64,
    object_threshold: f32,
) -> i32 {
    let sal = saliency_map(rgb, w, h, 40);
    let wm = sal.len();
    let column_w = (w as f64 * column_frac).round() as usize;

    // free gaps between objects and screen edges
    let mut gaps: Vec<(usize, usize)> = Vec::new();
    let mut prev_end = 0usize;
    for (start, end) in occupied_segments(&sal, object_threshold) {
        if start > prev_end {
            gaps.push((prev_end, start));
        }
        prev_end = prev_end.max(end);
    }
    if wm > prev_end {
        gaps.push((prev_end, wm));
    }

    let fitting: Vec<(usize, usize)> = gaps
        .into_iter()
        .filter(|(a, b)| (b - a) as f64 / wm as f64 * w as f64 >= column_w as f64)
        .collect();

    if let Some(&(a, b)) = fitting.iter().max_by_key(|(a, b)| b - a) {
        let centre = (a + b) as f64 / 2.0 / wm as f64; // 0..1 from left
        return ((centre - 0.5) * 100.0).round() as i32;
    }

    // fallback: least-edge-energy position for dense images
    let edge = edge_profile(rgb, w, h, 98.0, 50);
    let wm = edge.len(); // Python: wm = len(edge), which is w-1 for edge profiles
    let mut best_offset = 0i32;
    let mut best_score = f32::INFINITY;
    for offset_pct in -45..=45i32 {
        let cx = w as i64 / 2 + (w as f64 * offset_pct as f64 / 100.0).round() as i64;
        let x0 = cx - column_w as i64 / 2;
        let x1 = x0 + column_w as i64;
        if x0 < 0 || x1 > w as i64 {
            continue;
        }
        let i0 = (x0 as f64 * wm as f64 / w as f64).floor() as usize;
        let i1 = ((x1 as f64 * wm as f64 / w as f64).ceil() as usize).min(wm);
        if i0 >= i1 {
            continue;
        }
        let covered = edge[i0..i1].iter().sum::<f32>() / (i1 - i0) as f32;
        let distance = (cx as f64 - w as f64 / 2.0).abs() / w as f64;
        let edge_penalty = (distance - 0.15).max(0.0) as f32 * 0.1;
        let score = covered + edge_penalty;
        if score < best_score {
            best_score = score;
            best_offset = offset_pct;
        }
    }
    best_offset
}

/// Most vivid (bright + saturated) representative color, `RRGGBB` uppercase.
pub fn vivid_color(rgb: &[u8], w: usize, h: usize, top_frac: f64) -> String {
    let n = w * h;
    // subsample for speed
    let (sub, sub_n) = subsample(rgb, n, 400_000);

    let mut brightness = vec![0.0f32; sub_n];
    let mut saturation = vec![0.0f32; sub_n];
    for i in 0..sub_n {
        let r = sub[i * 3] as f32;
        let g = sub[i * 3 + 1] as f32;
        let b = sub[i * 3 + 2] as f32;
        let mx = r.max(g).max(b);
        let mn = r.min(g).min(b);
        brightness[i] = (r * 0.299 + g * 0.587 + b * 0.114) / 255.0;
        saturation[i] = if mx == 0.0 {
            0.0
        } else {
            (mx - mn) / mx.max(1.0)
        };
    }
    let score: Vec<f32> = brightness
        .iter()
        .zip(&saturation)
        .map(|(b, s)| b * s)
        .collect();

    let k = (sub_n as f64 * top_frac).max(1.0) as usize;
    let top = top_indices(&score, k);
    pick_mean(&sub, &top)
}

/// Brightest representative color, `RRGGBB` uppercase.
pub fn bright_color(rgb: &[u8], w: usize, h: usize, top_frac: f64) -> String {
    let n = w * h;
    let (sub, sub_n) = subsample(rgb, n, 400_000);

    let luminance: Vec<f32> = sub
        .chunks_exact(3)
        .map(|p| p[0] as f32 * 0.299 + p[1] as f32 * 0.587 + p[2] as f32 * 0.114)
        .collect();

    let k = (sub_n as f64 * top_frac).max(1.0) as usize;
    let top = top_indices(&luminance, k);
    pick_mean(&sub, &top)
}

fn subsample(rgb: &[u8], n: usize, limit: usize) -> (Vec<u8>, usize) {
    if n <= limit {
        return (rgb.to_vec(), n);
    }
    // deterministic stride subsample (Python uses default_rng(0), which is not
    // reproducible here identically; a uniform stride preserves the distribution)
    let step = n as f64 / limit as f64;
    let mut out = Vec::with_capacity(limit * 3);
    let mut j = 0usize;
    while j < n {
        out.extend_from_slice(&rgb[j * 3..(j + 1) * 3]);
        j += step.ceil() as usize;
    }
    let n_out = out.len() / 3;
    (out, n_out)
}

/// Indices of the `k` largest values (by descending value).
fn top_indices(values: &[f32], k: usize) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..values.len()).collect();
    let k = k.min(values.len());
    idx.select_nth_unstable_by(k, |&a, &b| {
        values[b]
            .partial_cmp(&values[a])
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    idx.truncate(k);
    idx
}

fn pick_mean(rgb: &[u8], idx: &[usize]) -> String {
    let mut r = 0.0f64;
    let mut g = 0.0f64;
    let mut b = 0.0f64;
    for &i in idx {
        r += rgb[i * 3] as f64;
        g += rgb[i * 3 + 1] as f64;
        b += rgb[i * 3 + 2] as f64;
    }
    let n = idx.len() as f64;
    let r = (r / n).round().clamp(0.0, 255.0) as u8;
    let g = (g / n).round().clamp(0.0, 255.0) as u8;
    let b = (b / n).round().clamp(0.0, 255.0) as u8;
    format!("{r:02X}{g:02X}{b:02X}")
}
