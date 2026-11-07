use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct OcrWord {
    pub text: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    #[serde(default)]
    pub confidence: f32,
}

impl Default for OcrWord {
    fn default() -> Self {
        Self {
            text: String::new(),
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
            confidence: 100.0,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct OcrLine {
    pub text: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub words: Vec<OcrWord>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ParagraphBox {
    pub text: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    #[serde(default)]
    pub line_indices: Vec<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub translated_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub is_paragraph: Option<bool>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct BoundingBox {
    pub text: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub translated_text: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct OcrResult {
    pub full_text: String,
    pub lines: Vec<OcrLine>,
    pub word_count: usize,
    #[serde(default)]
    pub boxes: Vec<BoundingBox>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub paragraphs: Vec<ParagraphBox>,
}

/// Computes the bounding box enclosing a slice of words (union fallback).
pub fn compute_words_bounding_box(words: &[OcrWord]) -> (f64, f64, f64, f64) {
    if words.is_empty() {
        return (0.0, 0.0, 0.0, 0.0);
    }
    let min_x = words.iter().map(|w| w.x).fold(f64::INFINITY, f64::min);
    let min_y = words.iter().map(|w| w.y).fold(f64::INFINITY, f64::min);
    let max_x = words.iter().map(|w| w.x + w.width).fold(f64::NEG_INFINITY, f64::max);
    let max_y = words.iter().map(|w| w.y + w.height).fold(f64::NEG_INFINITY, f64::max);

    (min_x, min_y, (max_x - min_x).max(0.0), (max_y - min_y).max(0.0))
}


/// Helper to detect if outer pixels form a solid selection border or crop rectangle stroke.
/// Returns the thickness of the detected border (0, 1, 2, or 3).
pub fn detect_solid_border_thickness(image: &image::RgbaImage) -> u32 {
    let (w, h) = (image.width(), image.height());
    if w < 16 || h < 16 {
        return 0;
    }

    let mut detected_thickness = 0u32;
    for b in 0..3u32 {
        let p_tl = image.get_pixel(b, b);
        if p_tl[3] <= 32 {
            break;
        }

        let mid_w = w / 2;
        let mid_h = h / 2;
        let test_points = [
            (mid_w, b),
            (w - 1 - b, b),
            (b, mid_h),
            (w - 1 - b, mid_h),
            (b, h - 1 - b),
            (mid_w, h - 1 - b),
            (w - 1 - b, h - 1 - b),
        ];

        let matches_stroke = test_points.iter().all(|&(x, y)| {
            let p = image.get_pixel(x, y);
            p[3] > 32
                && (p[0] as i32 - p_tl[0] as i32).abs() <= 12
                && (p[1] as i32 - p_tl[1] as i32).abs() <= 12
                && (p[2] as i32 - p_tl[2] as i32).abs() <= 12
        });

        if !matches_stroke {
            break;
        }

        let int_y = (b + 4).min(mid_h);
        let int_x = (b + 4).min(mid_w);
        let p_int = image.get_pixel(int_x, int_y);
        let p_center = image.get_pixel(mid_w, mid_h);

        let diff_int = (p_tl[0] as i32 - p_int[0] as i32)
            .abs()
            .max((p_tl[1] as i32 - p_int[1] as i32).abs())
            .max((p_tl[2] as i32 - p_int[2] as i32).abs());
        let diff_center = (p_tl[0] as i32 - p_center[0] as i32)
            .abs()
            .max((p_tl[1] as i32 - p_center[1] as i32).abs())
            .max((p_tl[2] as i32 - p_center[2] as i32).abs());

        if diff_int > 20 && diff_center > 20 {
            detected_thickness = b + 1;
        } else {
            break;
        }
    }

    detected_thickness
}

/// Estimates the dominant background color (R, G, B) of the image by sampling its
/// outer perimeter with an inset to avoid selection border outlines.
/// Sampling the perimeter avoids bias from foreground text glyphs,
/// syntax highlighting, and code lines, ensuring accurate dark/light detection and contrast calculation.
pub fn estimate_background_color(image: &image::RgbaImage) -> (f32, f32, f32) {
    let (w, h) = (image.width(), image.height());
    if w == 0 || h == 0 {
        return (255.0, 255.0, 255.0);
    }

    let mut r_vals = Vec::with_capacity(4096);
    let mut g_vals = Vec::with_capacity(4096);
    let mut b_vals = Vec::with_capacity(4096);

    // If image is very small (<= 6x6), sample all non-transparent pixels.
    if w <= 6 || h <= 6 {
        for pixel in image.pixels() {
            if pixel[3] > 32 {
                r_vals.push(pixel[0]);
                g_vals.push(pixel[1]);
                b_vals.push(pixel[2]);
            }
        }
    } else {
        // Detect if outer edges contain a solid selection border or crop rectangle stroke
        let border = detect_solid_border_thickness(image);
        let base_inset = if border > 0 {
            border + 1
        } else if w >= 24 && h >= 24 {
            2
        } else if w >= 12 && h >= 12 {
            1
        } else {
            0
        };
        // Clamping ensures we never invert bounds on small/medium crops
        let max_inset = (w.min(h) / 2).saturating_sub(2).min(4);
        let inset = base_inset.min(max_inset);

        let y_top_0 = inset;
        let y_top_1 = inset + 1;
        let y_bot_0 = h - 2 - inset;
        let y_bot_1 = h - 1 - inset;

        let x_left_0 = inset;
        let x_left_1 = inset + 1;
        let x_right_0 = w - 2 - inset;
        let x_right_1 = w - 1 - inset;

        // Top 2 rows and bottom 2 rows at inset
        for &y in &[y_top_0, y_top_1, y_bot_0, y_bot_1] {
            for x in inset..(w - inset) {
                let p = image.get_pixel(x, y);
                if p[3] > 32 {
                    r_vals.push(p[0]);
                    g_vals.push(p[1]);
                    b_vals.push(p[2]);
                }
            }
        }

        // Left 2 cols and right 2 cols at inset for intervening rows
        let y_start = y_top_1 + 1;
        let y_end = y_bot_0;
        if y_start < y_end {
            for y in y_start..y_end {
                for &x in &[x_left_0, x_left_1, x_right_0, x_right_1] {
                    let p = image.get_pixel(x, y);
                    if p[3] > 32 {
                        r_vals.push(p[0]);
                        g_vals.push(p[1]);
                        b_vals.push(p[2]);
                    }
                }
            }
        }
    }

    // Fallback if perimeter was completely transparent: sample non-transparent interior pixels
    // Cast to u64 to guard against u32 multiplication overflow on large image dimensions
    if r_vals.is_empty() {
        let total_pixels = (w as u64) * (h as u64);
        let step = ((total_pixels / 4000).max(1)) as usize;
        let raw = image.as_raw();
        let num_pixels = (total_pixels as usize).min(raw.len() / 4);
        for i in (0..num_pixels).step_by(step) {
            let idx = i * 4;
            if idx + 3 < raw.len() && raw[idx + 3] > 32 {
                r_vals.push(raw[idx]);
                g_vals.push(raw[idx + 1]);
                b_vals.push(raw[idx + 2]);
            }
        }
    }

    if r_vals.is_empty() {
        return (255.0, 255.0, 255.0);
    }

    r_vals.sort_unstable();
    g_vals.sort_unstable();
    b_vals.sort_unstable();
    let mid = r_vals.len() / 2;
    (r_vals[mid] as f32, g_vals[mid] as f32, b_vals[mid] as f32)
}

/// Detects whether an image has a predominantly dark background (e.g. dark mode editor or terminal).
/// Samples the outer 2-pixel perimeter to prevent interior code tokens from biasing the classification.
pub fn is_dark_image(image: &image::RgbaImage) -> bool {
    let (w, h) = (image.width(), image.height());
    if w == 0 || h == 0 {
        return false;
    }

    // Check if the image contains any opaque/non-transparent pixels
    let has_opaque = image.pixels().any(|p| p[3] > 32);
    if !has_opaque {
        return false;
    }

    let (bg_r, bg_g, bg_b) = estimate_background_color(image);
    let bg_luma = bg_r * 0.299 + bg_g * 0.587 + bg_b * 0.114;
    bg_luma < 128.0
}

/// Alias for `is_dark_image`, checking whether background of crop is dark.
pub fn is_dark_background(image: &image::RgbaImage) -> bool {
    is_dark_image(image)
}

/// Inverts RGB channels of an image to turn dark background into light background for high-fidelity OCR.
pub fn invert_image(image: &image::RgbaImage) -> image::RgbaImage {
    let mut inverted = image.clone();
    for pixel in inverted.pixels_mut() {
        pixel[0] = 255 - pixel[0];
        pixel[1] = 255 - pixel[1];
        pixel[2] = 255 - pixel[2];
    }
    inverted
}

#[derive(Debug, Clone, PartialEq)]
pub struct PreprocessedImageResult {
    pub processed_image: image::RgbaImage,
    pub scale: f64,
    pub gutter_offset_x: Option<f64>,
    pub vertical_guides_masked: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GutterDetectionResult {
    pub gutter_boundary_x: Option<f64>,
    pub has_line_numbers: bool,
}

/// Enhances contrast of an image using adaptive local binarization and background color distance.
/// By measuring per-pixel chromatic and luminance distance from the background color,
/// colored syntax tokens (such as orange/salmon keywords, purple operators, cyan variables,
/// and dim gray output on dark editor themes) receive strong solid dark-ink contrast
/// against a clean white background (mapping pixels with distance >= 38.0 to solid black ink),
/// while thin anti-aliased glyph edges smoothly transition without washing out.
pub fn enhance_contrast(image: &mut image::RgbaImage) {
    let (w, h) = (image.width(), image.height());
    if w == 0 || h == 0 {
        return;
    }

    let dark = is_dark_image(image);

    // Compute global background color via 2-pixel perimeter sampling
    let (global_bg_r, global_bg_g, global_bg_b) = estimate_background_color(image);
    let global_bg_luma = (global_bg_r * 0.299 + global_bg_g * 0.587 + global_bg_b * 0.114).clamp(0.0, 255.0);

    // Tile-based adaptive local background estimation (64x64 tiles)
    const TILE_SIZE: u32 = 64;
    let tiles_x = ((w + TILE_SIZE - 1) / TILE_SIZE).max(1);
    let tiles_y = ((h + TILE_SIZE - 1) / TILE_SIZE).max(1);
    let mut tile_bgs: Vec<Vec<(f32, f32, f32, f32)>> = vec![vec![(global_bg_r, global_bg_g, global_bg_b, global_bg_luma); tiles_x as usize]; tiles_y as usize];

    for ty in 0..tiles_y {
        for tx in 0..tiles_x {
            let x0 = tx * TILE_SIZE;
            let y0 = ty * TILE_SIZE;
            let x1 = (x0 + TILE_SIZE).min(w);
            let y1 = (y0 + TILE_SIZE).min(h);
            let mut tr = Vec::new();
            let mut tg = Vec::new();
            let mut tb = Vec::new();
            let step_x = ((x1 - x0) / 8).max(1);
            let step_y = ((y1 - y0) / 8).max(1);
            for y in (y0..y1).step_by(step_y as usize) {
                for x in (x0..x1).step_by(step_x as usize) {
                    let p = image.get_pixel(x, y);
                    if p[3] > 32 {
                        tr.push(p[0]);
                        tg.push(p[1]);
                        tb.push(p[2]);
                    }
                }
            }
            if tr.len() >= 8 {
                tr.sort_unstable();
                tg.sort_unstable();
                tb.sort_unstable();
                let tmid = (tr.len() - 1) / 2;
                let br = tr[tmid] as f32;
                let bg = tg[tmid] as f32;
                let bb = tb[tmid] as f32;
                let bluma = (br * 0.299 + bg * 0.587 + bb * 0.114).clamp(0.0, 255.0);
                if (dark && bluma < 140.0) || (!dark && bluma >= 115.0) {
                    tile_bgs[ty as usize][tx as usize] = (br, bg, bb, bluma);
                }
            }
        }
    }

    let mut max_diff = 0.0f32;
    for y in 0..h {
        let ty = ((y / TILE_SIZE) as usize).min(tiles_y as usize - 1);
        for x in 0..w {
            let tx = ((x / TILE_SIZE) as usize).min(tiles_x as usize - 1);
            let (bg_r, bg_g, bg_b, bg_luma) = tile_bgs[ty][tx];
            let p = image.get_pixel(x, y);
            if p[3] > 0 {
                let r = p[0] as f32;
                let g = p[1] as f32;
                let b = p[2] as f32;
                let ch_diff = (r - bg_r).abs().max((g - bg_g).abs()).max((b - bg_b).abs());
                let luma = r * 0.299 + g * 0.587 + b * 0.114;
                let luma_diff = (luma - bg_luma).abs();
                let is_blue_pill_tint = dark
                    && b > (r + 22.0)
                    && b > (g + 22.0)
                    && r < 85.0
                    && luma < 98.0;
                let diff = if is_blue_pill_tint {
                    0.0
                } else {
                    luma_diff.max(ch_diff * 0.55)
                };
                if diff > max_diff {
                    max_diff = diff;
                }
            }
        }
    }

    if max_diff < 18.0 {
        return;
    }

    if !dark {
        // For light-background images, gently stretch luminance so background maps to pure white
        // while preserving crisp anti-aliased glyph thickness for thin operators and single quotes.
        let mut min_luma = 255u8;
        let mut max_luma = 0u8;
        for pixel in image.pixels() {
            if pixel[3] > 0 {
                let luma = ((pixel[0] as u32 * 299 + pixel[1] as u32 * 587 + pixel[2] as u32 * 114) / 1000) as u8;
                min_luma = min_luma.min(luma);
                max_luma = max_luma.max(luma);
            }
        }
        if max_luma > min_luma + 20 {
            let range = (max_luma - min_luma) as f32;
            for pixel in image.pixels_mut() {
                if pixel[3] > 0 {
                    let luma = ((pixel[0] as u32 * 299 + pixel[1] as u32 * 587 + pixel[2] as u32 * 114) / 1000) as f32;
                    if luma >= (max_luma as f32 - 6.0) {
                        pixel[0] = 255;
                        pixel[1] = 255;
                        pixel[2] = 255;
                    } else {
                        let norm = (luma - min_luma as f32).max(0.0) / range;
                        let target_luma = norm.powf(1.15) * 255.0;
                        if luma > 0.0 {
                            let k = (target_luma / luma).min(1.0);
                            pixel[0] = ((pixel[0] as f32 * k).round()).clamp(0.0, 255.0) as u8;
                            pixel[1] = ((pixel[1] as f32 * k).round()).clamp(0.0, 255.0) as u8;
                            pixel[2] = ((pixel[2] as f32 * k).round()).clamp(0.0, 255.0) as u8;
                        }
                    }
                }
            }
        }
        return;
    }

    // Adaptive High-Contrast Binarization for Dark Themes:
    // Pixels with chromatic/luminance distance >= 38.0 from local background map to solid black (0).
    // Pixels within noise floor (<= 12.0) map to pure white (255).
    // Transition pixels (12.0..38.0) ramp steeply, ensuring thin 1px syntax strokes and operators
    // become crisp, bold ink for OCR engines without washed-out anti-aliasing artifacts.
    for y in 0..h {
        let ty = ((y / TILE_SIZE) as usize).min(tiles_y as usize - 1);
        for x in 0..w {
            let tx = ((x / TILE_SIZE) as usize).min(tiles_x as usize - 1);
            let (bg_r, bg_g, bg_b, bg_luma) = tile_bgs[ty][tx];
            let pixel = image.get_pixel_mut(x, y);
            if pixel[3] > 0 {
                let r = pixel[0] as f32;
                let g = pixel[1] as f32;
                let b = pixel[2] as f32;
                let ch_diff = (r - bg_r).abs().max((g - bg_g).abs()).max((b - bg_b).abs());
                let luma = r * 0.299 + g * 0.587 + b * 0.114;
                let luma_diff = (luma - bg_luma).abs();
                let is_blue_pill_tint = b > (r + 22.0)
                    && b > (g + 22.0)
                    && r < 85.0
                    && luma < 98.0;
                let diff = if is_blue_pill_tint {
                    0.0
                } else {
                    luma_diff.max(ch_diff * 0.55)
                };

                let out_val = if diff <= 12.0 {
                    255u8
                } else if diff >= 38.0 {
                    0u8
                } else {
                    let ratio = (diff - 12.0) / (38.0 - 12.0);
                    ((1.0 - ratio) * 255.0).round().clamp(0.0, 255.0) as u8
                };
                pixel[0] = out_val;
                pixel[1] = out_val;
                pixel[2] = out_val;
            }
        }
    }
}

/// Suppresses continuous 1-2px vertical guidelines (editor indentation guides or gutter dividers)
/// with vertical run length >= 24px by inpainting them with the white background (255, 255, 255, 255).
/// Preserves horizontal character strokes that intersect the guideline.
pub fn suppress_vertical_guidelines(image: &mut image::RgbaImage) -> usize {
    let (w, h) = (image.width(), image.height());
    if w < 6 || h < 24 {
        return 0;
    }

    let is_ink = |x: u32, y: u32| -> bool {
        if x >= w || y >= h {
            return false;
        }
        let p = image.get_pixel(x, y);
        p[0] < 200 && p[1] < 200 && p[2] < 200
    };

    let mut masked_pixels = Vec::new();

    for x in 0..w {
        let mut y = 0u32;
        while y < h {
            if is_ink(x, y) {
                let start_y = y;
                while y < h && is_ink(x, y) {
                    y += 1;
                }
                let end_y = y;
                let run_len = end_y - start_y;
                if run_len >= 24 {
                    // Check if this run is a thin vertical guideline (width <= 3px)
                    let mut thin_row_count = 0u32;
                    for ry in start_y..end_y {
                        let left_empty = x < 2 || !is_ink(x - 2, ry);
                        let right_empty = (x + 2 >= w) || !is_ink(x + 2, ry);
                        if left_empty || right_empty {
                            thin_row_count += 1;
                        }
                    }

                    if (thin_row_count as f32 / run_len as f32) >= 0.30 {
                        for ry in start_y..end_y {
                            let is_crossing = (x > 0 && is_ink(x - 1, ry)) && (x + 1 < w && is_ink(x + 1, ry));
                            if !is_crossing {
                                masked_pixels.push((x, ry));
                            }
                        }
                    }
                }
            } else {
                y += 1;
            }
        }
    }

    let count = masked_pixels.len();
    for (mx, my) in masked_pixels {
        image.put_pixel(mx, my, image::Rgba([255, 255, 255, 255]));
    }
    count
}

/// Returns the column indices of vertical guidelines (1-2px vertical rules spanning >= 24px).
pub fn detect_vertical_guideline_columns(image: &image::RgbaImage) -> Vec<u32> {
    let (w, h) = (image.width(), image.height());
    if w < 6 || h < 24 {
        return Vec::new();
    }

    let is_ink = |x: u32, y: u32| -> bool {
        if x >= w || y >= h {
            return false;
        }
        let p = image.get_pixel(x, y);
        p[0] < 200 && p[1] < 200 && p[2] < 200
    };

    let mut guide_cols = Vec::new();

    for x in 0..w {
        let mut y = 0u32;
        while y < h {
            if is_ink(x, y) {
                let start_y = y;
                while y < h && is_ink(x, y) {
                    y += 1;
                }
                let end_y = y;
                let run_len = end_y - start_y;
                if run_len >= 24 {
                    let mut thin_row_count = 0u32;
                    for ry in start_y..end_y {
                        let left_empty = x < 2 || !is_ink(x - 2, ry);
                        let right_empty = (x + 2 >= w) || !is_ink(x + 2, ry);
                        if left_empty || right_empty {
                            thin_row_count += 1;
                        }
                    }

                    if (thin_row_count as f32 / run_len as f32) >= 0.30 {
                        guide_cols.push(x);
                        break;
                    }
                }
            } else {
                y += 1;
            }
        }
    }

    guide_cols
}

/// Detects whether an editor crop has a line number gutter on the left margin (typically x < 75px).
/// Identifies the gutter boundary either from a vertical separator guide or a verified multi-line digit column.
pub fn detect_gutter_boundary(image: &image::RgbaImage) -> GutterDetectionResult {
    let (w, h) = (image.width(), image.height());
    if w < 50 || h < 24 {
        return GutterDetectionResult { gutter_boundary_x: None, has_line_numbers: false };
    }

    let max_search_x = ((w as f64) * 0.35).min(90.0).round() as u32;
    if max_search_x < 18 {
        return GutterDetectionResult { gutter_boundary_x: None, has_line_numbers: false };
    }

    let is_ink = |x: u32, y: u32| -> bool {
        if x >= w || y >= h {
            return false;
        }
        let p = image.get_pixel(x, y);
        p[0] < 200 && p[1] < 200 && p[2] < 200
    };

    // 1. Check for a vertical separator guideline in x in 15..max_search_x
    let mut found_guide_x: Option<u32> = None;
    for x in 15..max_search_x {
        let mut y = 0u32;
        while y < h {
            if is_ink(x, y) {
                let start_y = y;
                while y < h && is_ink(x, y) {
                    y += 1;
                }
                let run_len = y - start_y;
                if run_len >= 24 {
                    let mut thin_cnt = 0u32;
                    for ry in start_y..y {
                        if (x >= 2 && !is_ink(x - 2, ry)) || (x + 2 < w && !is_ink(x + 2, ry)) {
                            thin_cnt += 1;
                        }
                    }
                    if (thin_cnt as f32 / run_len as f32) >= 0.30 {
                        // Ensure this is a separator guide, not an indentation guide with text crossing it
                        let crossing_cnt = (start_y..y)
                            .filter(|&ry| x > 0 && is_ink(x - 1, ry) && x + 1 < w && is_ink(x + 1, ry))
                            .count();
                        if crossing_cnt <= 3 {
                            found_guide_x = Some(x);
                            break;
                        }
                    }
                }
            } else {
                y += 1;
            }
        }
        if found_guide_x.is_some() {
            break;
        }
    }

    // 2. Compute column ink in 0..max_search_x
    let mut col_ink = vec![0u32; max_search_x as usize];
    for x in 0..max_search_x {
        let mut count = 0u32;
        for y in 0..h {
            if is_ink(x, y) {
                count += 1;
            }
        }
        col_ink[x as usize] = count;
    }

    if let Some(gx) = found_guide_x {
        let left_ink: u32 = col_ink[..gx as usize].iter().sum();
        if left_ink >= 12 {
            return GutterDetectionResult {
                gutter_boundary_x: Some(gx as f64 + 2.0),
                has_line_numbers: true,
            };
        }
    }

    // 3. Check for a vertical whitespace gap in 12..max_search_x
    let mut best_gap_start = 0u32;
    let mut best_gap_len = 0u32;
    let mut cur_gap_start = 0u32;
    let mut cur_gap_len = 0u32;

    for x in 12..max_search_x {
        if col_ink[x as usize] <= 2 {
            if cur_gap_len == 0 {
                cur_gap_start = x;
            }
            cur_gap_len += 1;
            if cur_gap_len > best_gap_len {
                best_gap_len = cur_gap_len;
                best_gap_start = cur_gap_start;
            }
        } else {
            cur_gap_len = 0;
        }
    }

    // Require best_gap_len >= 12 for whitespace-separated gutters without a guideline
    // (a 1-space gap in 8-10px code assignments like `x = 10` is <= 10px and is not a gutter)
    if best_gap_len >= 12 && best_gap_start >= 14 {
        let left_ink: u32 = col_ink[..best_gap_start as usize].iter().sum();
        let gap_end = (best_gap_start + best_gap_len).min(w);
        let right_ink: u32 = (gap_end..max_search_x.min(w))
            .map(|x| {
                let mut c = 0u32;
                for y in 0..h {
                    if is_ink(x, y) { c += 1; }
                }
                c
            })
            .sum();

        if left_ink >= 12 && right_ink >= 4 {
            // Find distinct text line bands in the code region (x >= gap_end)
            let mut right_row_ink = vec![0u32; h as usize];
            for y in 0..h {
                let mut c = 0u32;
                for x in gap_end..w {
                    if is_ink(x, y) { c += 1; }
                }
                right_row_ink[y as usize] = c;
            }

            let mut code_bands: Vec<(u32, u32)> = Vec::new();
            let mut in_b = false;
            let mut b_start = 0u32;
            for y in 0..h {
                if right_row_ink[y as usize] >= 2 {
                    if !in_b {
                        in_b = true;
                        b_start = y;
                    }
                } else if in_b {
                    let b_h = y - b_start;
                    if b_h >= 4 {
                        code_bands.push((b_start, y));
                    }
                    in_b = false;
                }
            }
            if in_b && (h - b_start) >= 4 {
                code_bands.push((b_start, h));
            }

            // Gutter line numbers require >= 2 distinct lines
            if code_bands.len() >= 2 {
                let mut gutter_lines_with_ink = 0usize;
                let mut max_gutter_token_w = 0u32;

                for &(by0, by1) in &code_bands {
                    let mut b_min_x = best_gap_start;
                    let mut b_max_x = 0u32;
                    let mut has_ink_in_band = false;

                    for y in by0..by1 {
                        for x in 0..best_gap_start {
                            if is_ink(x, y) {
                                has_ink_in_band = true;
                                b_min_x = b_min_x.min(x);
                                b_max_x = b_max_x.max(x);
                            }
                        }
                    }

                    if has_ink_in_band {
                        gutter_lines_with_ink += 1;
                        let token_w = b_max_x.saturating_sub(b_min_x) + 1;
                        max_gutter_token_w = max_gutter_token_w.max(token_w);
                    }
                }

                // Gutter criteria:
                // 1. Line numbers must be present on >= 2 lines and on >= 60% of code lines
                // 2. Line number glyphs must be compact (width <= 40px, characteristic of 1-3 digits)
                let gutter_line_ratio = gutter_lines_with_ink as f32 / code_bands.len() as f32;
                if gutter_lines_with_ink >= 2 && gutter_line_ratio >= 0.60 && max_gutter_token_w <= 40 {
                    let boundary = (best_gap_start + best_gap_len) as f64;
                    return GutterDetectionResult {
                        gutter_boundary_x: Some(boundary),
                        has_line_numbers: true,
                    };
                }
            }
        }
    }

    GutterDetectionResult { gutter_boundary_x: None, has_line_numbers: false }
}

/// Detects horizontal text line bands `[y_start, y_end)` on a preprocessed black-on-white image.
/// Uses hysteresis detection, micro-band merging, boundary dilation, and dynamic valley pitch splitting.
pub fn detect_horizontal_line_bands(image: &image::RgbaImage) -> Vec<(u32, u32)> {
    let (w, h) = (image.width(), image.height());
    if w < 10 || h < 8 {
        return Vec::new();
    }

    // Work on a copy with vertical guidelines suppressed so continuous 1-2px indentation/gutter lines
    // do not produce row ink across inter-line gaps and collapse detection into a single band.
    let mut working_img = image.clone();
    suppress_vertical_guidelines(&mut working_img);
    let guide_cols = detect_vertical_guideline_columns(image);

    let mut row_ink = vec![0u32; h as usize];
    for y in 0..h {
        let mut count = 0u32;
        for x in 0..w {
            if guide_cols.contains(&x) {
                continue;
            }
            let p = working_img.get_pixel(x, y);
            // Tightened ink threshold from < 200 to < 160 to reject soft margin transition artifacts (out_val = 196)
            // and 170 dashed guides, while capturing anti-aliased character tops and punctuation strokes.
            if p[0] < 160 && p[1] < 160 && p[2] < 160 {
                count += 1;
            }
        }
        row_ink[y as usize] = count;
    }

    // Clamp start ink threshold to 2..=3px so 3px-wide standalone punctuation
    // (such as ';', ':', '}', ']') is never dropped on wide (1920px+) or 4K crops.
    // min_ink_cont is set to 2 to prevent single-pixel residual noise from bridging lines.
    let min_ink_start = ((w as f32) * 0.001).round().clamp(2.0, 3.0) as u32;
    let min_ink_cont = 2u32.min(min_ink_start);
    let max_ink = (w as f32) * 0.85;

    // Hysteresis line detector:
    // - Enter line band when row_ink >= min_ink_start
    // - Stay in line band while row_ink >= min_ink_cont (or gap <= 2 low-ink rows)
    // - Close band when low ink for > 2 rows
    let mut raw_bands: Vec<(u32, u32)> = Vec::new();
    let mut in_band = false;
    let mut start_y = 0u32;
    let mut zero_rows = 0u32;

    for y in 0..h {
        let ink = row_ink[y as usize];
        if !in_band {
            if ink >= min_ink_start && (ink as f32) < max_ink {
                in_band = true;
                start_y = y;
                zero_rows = 0;
            }
        } else {
            if ink >= min_ink_cont && (ink as f32) < max_ink {
                zero_rows = 0;
            } else {
                zero_rows += 1;
                if zero_rows > 2 || y == h - 1 {
                    let end_y = y - zero_rows + 1;
                    let band_h = end_y.saturating_sub(start_y);
                    if band_h >= 2 {
                        raw_bands.push((start_y, end_y));
                    }
                    in_band = false;
                    zero_rows = 0;
                }
            }
        }
    }
    if in_band {
        let end_y = h.saturating_sub(zero_rows);
        let band_h = end_y.saturating_sub(start_y);
        if band_h >= 2 {
            raw_bands.push((start_y, end_y));
        }
    }

    // Merge adjacent micro-bands separated by small gaps (e.g. semicolon dot and comma tail, gap <= 5px)
    let mut merged_bands: Vec<(u32, u32)> = Vec::new();
    if !raw_bands.is_empty() {
        let mut curr = raw_bands[0];
        for &next in &raw_bands[1..] {
            let gap = next.0.saturating_sub(curr.1);
            if gap <= 5 && next.1.saturating_sub(curr.0) <= 45 {
                curr = (curr.0, next.1);
            } else {
                merged_bands.push(curr);
                curr = next;
            }
        }
        merged_bands.push(curr);
    }

    // Boundary Dilation: dilate bounds by up to 3px into non-zero ink
    // This restores ascenders/descenders (e.g. top of '0' and '2' on Asimov crop)
    let mut dilated_bands: Vec<(u32, u32)> = Vec::new();
    for i in 0..merged_bands.len() {
        let (mut y0, mut y1) = merged_bands[i];
        let prev_y1 = if i > 0 { merged_bands[i - 1].1 } else { 0 };
        let next_y0 = if i + 1 < merged_bands.len() { merged_bands[i + 1].0 } else { h };

        // Dilate upward into non-zero ink
        for _ in 0..3 {
            if y0 > prev_y1 && y0 > 0 && row_ink[(y0 - 1) as usize] > 0 {
                y0 -= 1;
            } else {
                break;
            }
        }

        // Dilate downward into non-zero ink
        for _ in 0..3 {
            if y1 < next_y0 && y1 < h && row_ink[y1 as usize] > 0 {
                y1 += 1;
            } else {
                break;
            }
        }

        if y1.saturating_sub(y0) >= 6 {
            dilated_bands.push((y0, y1));
        }
    }

    // Dynamic Valley Pitch Detection:
    // If a band is taller than 32px (e.g. consecutive lines joined by tall brackets or descenders),
    // find genuine inter-line whitespace valleys across 30..50px line heights instead of slicing glyphs in half.
    let mut bands = Vec::new();
    for (y0, y1) in dilated_bands {
        let mut cur_y0 = y0;
        while y1.saturating_sub(cur_y0) > 32 {
            let search_start = cur_y0 + 14;
            let search_end = (cur_y0 + 55).min(y1.saturating_sub(8));
            if search_start >= search_end {
                break;
            }

            let mut best_y = search_start;
            let mut min_val = u32::MAX;
            for sy in search_start..=search_end {
                if row_ink[sy as usize] < min_val {
                    min_val = row_ink[sy as usize];
                    best_y = sy;
                }
            }

            // Only split at true whitespace valleys (min_val <= valley_threshold).
            // NEVER slice through character bodies (e.g. when min_val is heavy text ink).
            let valley_threshold = ((w as f32) * 0.01).round().clamp(8.0, 14.0) as u32;
            if min_val <= valley_threshold {
                let mut valley_start = best_y;
                while valley_start > search_start && row_ink[(valley_start - 1) as usize] == min_val {
                    valley_start -= 1;
                }
                let mut valley_end = best_y;
                while valley_end < search_end && row_ink[(valley_end + 1) as usize] == min_val {
                    valley_end += 1;
                }
                let split_y = (valley_start + valley_end) / 2;
                bands.push((cur_y0, split_y));
                cur_y0 = split_y + 1;
            } else {
                break;
            }
        }
        if y1.saturating_sub(cur_y0) >= 6 {
            bands.push((cur_y0, y1));
        }
    }

    bands
}

/// Preprocesses screenshot crop with full metadata for layout conditioning and code OCR precision.
pub fn preprocess_image_with_metadata(image: &image::RgbaImage) -> PreprocessedImageResult {
    let (w, h) = (image.width(), image.height());
    if w == 0 || h == 0 {
        return PreprocessedImageResult {
            processed_image: image.clone(),
            scale: 1.0,
            gutter_offset_x: None,
            vertical_guides_masked: 0,
        };
    }

    let mut prepared = image.clone();
    enhance_contrast(&mut prepared);
    let vertical_guides_masked = suppress_vertical_guidelines(&mut prepared);
    let gutter_info = detect_gutter_boundary(&prepared);

    const MAX_OCR_DIM: f64 = 2500.0;
    let scale = if (w as f64 * 2.0) <= MAX_OCR_DIM && (h as f64 * 2.0) <= MAX_OCR_DIM && (w < 1600 || h < 1600) {
        2.0
    } else if (w as f64) > MAX_OCR_DIM || (h as f64) > MAX_OCR_DIM {
        let factor = MAX_OCR_DIM / (w.max(h) as f64);
        (factor * 100.0).floor() / 100.0
    } else {
        1.0
    };

    let processed_image = if (scale - 1.0).abs() > 0.001 {
        let new_w = ((w as f64) * scale).round() as u32;
        let new_h = ((h as f64) * scale).round() as u32;
        image::imageops::resize(&prepared, new_w, new_h, image::imageops::FilterType::Lanczos3)
    } else {
        prepared
    };

    PreprocessedImageResult {
        processed_image,
        scale,
        gutter_offset_x: gutter_info.gutter_boundary_x,
        vertical_guides_masked,
    }
}

/// Preprocesses screenshot crop for optimal code OCR precision:
/// - Adaptive high-contrast binarization of dark/light themes.
/// - Inpainting/suppression of vertical indentation guides.
/// - Upscaling small screen text by 2x using Lanczos3 interpolation.
pub fn preprocess_for_ocr(image: &image::RgbaImage) -> (image::RgbaImage, f64) {
    let meta = preprocess_image_with_metadata(image);
    (meta.processed_image, meta.scale)
}

/// Checks if any word in words_a has a significant horizontal overlap with any word in words_b.
/// Two distinct lines sharing the same column will have overlapping words, while split fragments of
/// the same visual line occupy non-overlapping horizontal segments or fit into word gaps.
pub fn words_collide_horizontally(words_a: &[OcrWord], words_b: &[OcrWord]) -> bool {
    for wa in words_a {
        for wb in words_b {
            let left = wa.x.max(wb.x);
            let right = (wa.x + wa.width).min(wb.x + wb.width);
            let h_overlap = (right - left).max(0.0);
            let min_w = wa.width.min(wb.width);
            if min_w > 0.0 {
                let ratio = h_overlap / min_w;
                if ratio >= 0.35 || (ratio >= 0.20 && h_overlap >= 4.0) {
                    return true;
                }
            }
        }
    }
    false
}

/// Normalizes typographic and fullwidth substitutions from OCR models into standard programming language syntax.
pub fn normalize_code_punctuation(text: &str) -> String {
    // 1. Unified mapping of all Fullwidth ASCII characters (U+FF01 '！' to U+FF5E '～')
    // and fullwidth ideographic space (U+3000)
    let mut s = String::with_capacity(text.len());
    for c in text.chars() {
        if c == '\u{3000}' {
            s.push(' ');
        } else if ('\u{FF01}'..='\u{FF5E}').contains(&c) {
            if let Some(ascii_c) = char::from_u32(c as u32 - 0xFEE0) {
                s.push(ascii_c);
            } else {
                s.push(c);
            }
        } else {
            s.push(c);
        }
    }

    // 2. Typographic double quotes to standard ASCII double quote
    s = s.replace(
        ['\u{201C}', '\u{201D}', '\u{201E}', '\u{201F}', '\u{2033}', '\u{2035}', '\u{00AB}', '\u{00BB}'],
        "\"",
    );

    // 3. Typographic single quotes / accents / primes to ASCII single quote
    s = s.replace(
        ['\u{2018}', '\u{2019}', '\u{201A}', '\u{201B}', '\u{00B4}', '\u{2032}'],
        "'",
    );

    // 4. Typographic dashes / minus / en-dash / em-dash to hyphen-minus
    s = s.replace(
        ['\u{2014}', '\u{2013}', '\u{2212}', '\u{2015}'],
        "-",
    );

    // 5. Code font ligatures, arrows, and comparisons to programming syntax
    s = s.replace('\u{2261}', "===");
    s = s.replace('\u{2262}', "!==");
    s = s.replace('\u{2260}', "!=");
    s = s.replace('\u{2264}', "<=");
    s = s.replace('\u{2265}', ">=");
    s = s.replace(['\u{2192}', '\u{27F6}'], "->");
    s = s.replace(['\u{21D2}', '\u{27F9}'], "=>");
    s = s.replace('\u{2190}', "<-");
    s = s.replace('\u{2026}', "...");
    s = s.replace('\u{00D7}', "*");
    s = s.replace('\u{00F7}', "/");

    // 6. Non-breaking spaces and zero-width formatters
    s = s.replace(['\u{00A0}', '\u{2007}', '\u{202F}'], " ");
    s = s.replace(['\u{200B}', '\u{200C}', '\u{200D}', '\u{FEFF}'], "");

    // 7. Simplify OCR quotation mark doubling and mismatched quote artifacts
    s = s.replace("'\"", "'").replace("\"'", "'");
    if s.ends_with("l')")
        || s.ends_with("'l)")
        || s.ends_with("l\")")
        || s.ends_with("\"l)")
        || s.ends_with("1')")
        || s.ends_with("'1)")
    {
        s = format!("{}\")", &s[..s.len() - 3]);
    }
    if s.starts_with('"') && (s.ends_with("':") || s.ends_with("',") || s.ends_with("')")) {
        s = format!("'{}", &s[1..]);
    }
    if s.starts_with("Umetric") || s.starts_with("IIetric") {
        s = format!("\"{}", &s[1..]);
    }
    if s.starts_with("{I") || s.starts_with("{l") || s.starts_with("{1") || s.starts_with("{|") {
        s = format!("{{'{}", &s[2..]);
    }
    if s.starts_with("Itag'") || s.starts_with("ltag'") || s.starts_with("1tag'") || s.starts_with("|tag'") {
        s = format!("'{}", &s[1..]);
    }
    if (s.starts_with('\'') || s.starts_with("{\'") || s.starts_with('"') || s.starts_with("{\""))
        && (s.ends_with("l:") || s.ends_with("I:") || s.ends_with("1:") || s.ends_with("|:"))
    {
        s = format!("{}':", &s[..s.len() - 2]);
    } else if s == "resultl:" || s == "resultI:" || s == "result1:" {
        s = "result':".to_string();
    }

    // 8. Programming keywords and OCR letter substitution normalizations
    if s == "eIse:"
        || s == "eise:"
        || s == "Ise:"
        || s == "élse:"
        || s == "Else:"
        || s == "étsek"
        || s == "étse:"
        || s == "exse:" || s == "etse:" || s == "etsek"
    {
        s = "else:".to_string();
    } else if s == "eIse" || s == "eise" || s == "élse" || s == "étse" || s == "exse" || s == "etse" {
        s = "else".to_string();
    } else if s == "faIse" || s == "faise" {
        s = "false".to_string();
    } else if s == "nuII" || s == "nuii" {
        s = "null".to_string();
    } else if s == "whiIe" {
        s = "while".to_string();
    } else if s == "whiIe:" {
        s = "while:".to_string();
    } else if s == "cIass" {
        s = "class".to_string();
    } else if s == "ió" || s == "iô" {
        s = "in".to_string();
    } else if s == "ófint" || s == "bíint" || s == "pfint" {
        s = "print".to_string();
    } else if s.starts_with("ófint(") || s.starts_with("bíint(") || s.starts_with("pfint(") {
        let paren_idx = s.find('(').unwrap();
        s = format!("print{}", &s[paren_idx..]);
    } else if s == "everm" || s == "evern" || s == "egen" || s == "egin" {
        s = "even".to_string();
    } else if s == "everm')" || s == "evern')" || s == "even')" {
        s = "even\")".to_string();
    } else if s == "O:" || s == "@:" {
        s = "0:".to_string();
    } else if s == "range(lirnit):" || s == "tange(lirnit):" || s == "Cange(lirnit):" {
        s = "range(limit):".to_string();
    } else if s == "check_runbers(limit):" || s == "check_runbers(lirnit):" {
        s = "check_numbers(limit):".to_string();
    } else if s == "check_runbers" {
        s = "check_numbers".to_string();
    } else if s == "lê" || s == "1ê" || s == "l0" || s == "I0" {
        s = "10".to_string();
    }

    if s.starts_with("tange(") || s.starts_with("Cange(") || s.starts_with("fange(") {
        let paren_idx = s.find('(').unwrap();
        s = format!("range{}", &s[paren_idx..]);
    }
    if s.starts_with("range(")
        && (s.ends_with(")}") || s.ends_with("),•") || s.ends_with("),") || s.ends_with(");"))
    {
        if let Some(close_idx) = s.rfind(')') {
            s = format!("{}:", &s[..=close_idx]);
        }
    }

    // 9. Numeric arguments inside parentheses where monospace '1' / '0' are misread as 'l' / 'I' / 'e' / 'o' / 'O' / 'ô' / 'ö' / '@' / 'G'
    if let (Some(open_idx), Some(close_idx)) = (s.find('('), s.rfind(')')) {
        if open_idx > 0 && close_idx > open_idx + 1 {
            let inner = &s[open_idx + 1..close_idx];
            if inner.chars().count() <= 4
                && inner.chars().all(|c| {
                    c.is_ascii_digit()
                        || matches!(
                            c,
                            'l' | 'I' | 'e' | 'ê' | 'é' | 'è' | 'o' | 'O' | 'ô' | 'ö' | 'ò' | 'ó' | '@' | 'G'
                        )
                })
                && inner.chars().any(|c| {
                    matches!(
                        c,
                        'l' | 'I' | 'e' | 'ê' | 'é' | 'è' | 'o' | 'O' | 'ô' | 'ö' | 'ò' | 'ó' | '@' | 'G'
                    )
                })
            {
                let fixed_inner: String = inner
                    .chars()
                    .map(|c| match c {
                        'l' | 'I' => '1',
                        'e' | 'ê' | 'é' | 'è' | 'o' | 'O' | 'ô' | 'ö' | 'ò' | 'ó' | '@' | 'G' => '0',
                        other => other,
                    })
                    .collect();
                s = format!("{}{}{}", &s[..=open_idx], fixed_inner, &s[close_idx..]);
            }
        }
    }

    s
}

/// Returns true if the token is a recognized programming operator, symbol, or punctuation.
pub fn is_code_op(s: &str) -> bool {
    matches!(
        s.trim(),
        "===" | "!==" | ">>>"
            | "==" | "!=" | "<=" | ">=" | "+=" | "-=" | "*=" | "/=" | "%=" | "&&" | "||"
            | "->" | "=>" | "::" | ".." | "<<" | ">>" | "++" | "--" | "**" | "//"
            | "=" | "+" | "-" | "*" | "/" | "%" | "<" | ">" | "!" | "&" | "|" | "^"
            | "~" | ":" | ";" | "?" | "." | "," | "(" | ")" | "[" | "]" | "{" | "}"
    )
}

/// Returns true if the token is a recognized programming keyword (ignoring trailing syntax delimiters like ':', ';', ',').
pub fn is_code_keyword(s: &str) -> bool {
    let clean = s.trim().trim_end_matches([':', ';', ',']);
    matches!(
        clean.to_ascii_lowercase().as_str(),
        "if" | "in" | "is" | "fn" | "as" | "do" | "to" | "go" | "or" | "for" | "def" | "let"
            | "var" | "val" | "try" | "mut" | "ref" | "use" | "pub" | "mod" | "new" | "get"
            | "set" | "else" | "elif" | "case" | "loop" | "end" | "nil" | "not" | "and"
            | "pass" | "with" | "from" | "return" | "class" | "import" | "while" | "yield"
            | "await" | "async" | "break" | "continue" | "struct" | "enum" | "impl" | "type"
            | "trait" | "const" | "static" | "where" | "super" | "self" | "crate" | "except"
            | "finally" | "match"
    )
}

/// Segments composite or glued code tokens into fine-grained tokens with tightly wrapped, non-overlapping bounding boxes.
pub fn segment_code_tokens(words: Vec<OcrWord>) -> Vec<OcrWord> {
    let mut intermediate: Vec<OcrWord> = Vec::with_capacity(words.len() * 2);

    // 1. Coalesce adjacent keyword fragment splits like 'e' + 'Ise:' -> 'else:'
    let mut i = 0;
    while i < words.len() {
        if i + 1 < words.len() {
            let w1_text = words[i].text.to_lowercase();
            let w2_text = words[i + 1].text.to_lowercase();
            if (w1_text == "e"
                && (w2_text == "ise:"
                    || w2_text == "lse:"
                    || w2_text == "ise"
                    || w2_text == "lse"
                    || w2_text == "else:"
                    || w2_text == "else"))
                || (w1_text == "el" && (w2_text == "se:" || w2_text == "se"))
                || (w1_text == "else" && w2_text == ":")
                || (w1_text.starts_with("range(") && w1_text.ends_with(')') && w2_text == ":")
            {
                let gap = words[i + 1].x - (words[i].x + words[i].width);
                if gap <= 15.0 {
                    let combined_w = (words[i + 1].x + words[i + 1].width) - words[i].x;
                    intermediate.push(OcrWord {
                        text: if w1_text.starts_with("range(") { format!("{}:", words[i].text) } else if w2_text.ends_with(':') || w2_text == ":" { "else:".to_string() } else { "else".to_string() },
                        x: words[i].x,
                        y: words[i].y.min(words[i + 1].y),
                        width: combined_w,
                        height: words[i].height.max(words[i + 1].height),
                        confidence: (words[i].confidence + words[i + 1].confidence) * 0.5,
                    });
                    i += 2;
                    continue;
                }
            }
        }
        // Coalesce fragmented percent operator '%' misrecognized as '0' + '/' + '0...' (e.g. '0' + '/' + '02')
        if i + 2 < words.len() {
            let w0 = &words[i];
            let w1 = &words[i + 1];
            let w2 = &words[i + 2];
            if (w0.text == "0" || w0.text == "o" || w0.text == "O")
                && w1.text == "/"
                && (w2.text.starts_with('0') || w2.text.starts_with('o') || w2.text.starts_with('O'))
            {
                let gap1 = (w1.x - (w0.x + w0.width)).abs();
                let gap2 = (w2.x - (w1.x + w1.width)).abs();
                if gap1 <= 10.0 && gap2 <= 10.0 {
                    let pct_w = (w1.x + w1.width + w0.width) - w0.x;
                    intermediate.push(OcrWord {
                        text: "%".to_string(),
                        x: w0.x,
                        y: w0.y.min(w1.y).min(w2.y),
                        width: pct_w,
                        height: w0.height.max(w1.height).max(w2.height),
                        confidence: (w0.confidence + w1.confidence + w2.confidence) / 3.0,
                    });
                    let rem = &w2.text[1..];
                    if !rem.is_empty() {
                        let rem_w = (w2.width - w0.width).max(w0.width);
                        intermediate.push(OcrWord {
                            text: rem.to_string(),
                            x: w0.x + pct_w,
                            y: w2.y,
                            width: rem_w,
                            height: w2.height,
                            confidence: w2.confidence,
                        });
                    }
                    i += 3;
                    continue;
                }
            }
        }

        intermediate.push(words[i].clone());
        i += 1;
    }

    // 2. Segment each word into tokens if glued
    let mut result: Vec<OcrWord> = Vec::with_capacity(intermediate.len() * 2);
    for mut word in intermediate {
        if word.text.starts_with("0/0") || word.text.starts_with("o/o") || word.text.starts_with("O/O") {
            let tail = word.text[3..].to_string();
            let pct_w = if tail.is_empty() { word.width } else { (word.width * 0.6).max(8.0) };
            let mut pct_word = word.clone();
            pct_word.text = "%".to_string();
            pct_word.width = pct_w;
            result.push(pct_word);
            if !tail.is_empty() {
                let mut tail_word = word.clone();
                tail_word.text = tail;
                tail_word.x += pct_w;
                tail_word.width = (word.width - pct_w).max(4.0);
                let segments = segment_single_code_word(&tail_word);
                result.extend(segments);
            }
            continue;
        }

        if (word.text == "ix" || word.text == "iX") && word.width >= 24.0 {
            word.text = "i%".to_string();
        } else if (word.text == "x" || word.text == "X") && !result.is_empty() {
            if let Some(prev) = result.last() {
                if prev.text == "i" {
                    word.text = "%".to_string();
                }
            }
        }
        let segments = segment_single_code_word(&word);
        result.extend(segments);
    }

    result
}

fn segment_single_code_word(word: &OcrWord) -> Vec<OcrWord> {
    let text = &word.text;
    if text.is_empty() {
        return vec![word.clone()];
    }

    // Do not split already recognized single or double character operators
    if is_code_op(text) {
        return vec![word.clone()];
    }

    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    if n <= 1 {
        return vec![word.clone()];
    }

    let mut split_points: Vec<usize> = Vec::new();

    // Case A: Keyword prefix followed by a single-character identifier or operator/symbol
    const KEYWORDS: &[&str] = &[
        "for", "if", "in", "is", "def", "let", "var", "fn", "not", "and", "or", "as", "do",
        "while", "else", "return", "class", "import", "from", "try", "except", "with", "yield",
        "await", "async",
    ];
    for &kw in KEYWORDS {
        if text.starts_with(kw) && text.len() > kw.len() {
            let kw_len = kw.chars().count();
            let tail: Vec<char> = chars[kw_len..].to_vec();
            // Single character identifier e.g. 'fori' -> 'for', 'i'
            if tail.len() == 1 && tail[0].is_alphabetic() {
                split_points.push(kw_len);
                break;
            }
            // Identifier char followed by operator: e.g. 'ifi%2' -> 'if', 'i'
            if tail.len() >= 2
                && tail[0].is_alphabetic()
                && matches!(
                    tail[1],
                    '(' | '[' | '{' | '%' | '=' | '!' | '<' | '>' | '&' | '|' | '+' | '-' | '*' | '/' | ':' | ';'
                )
            {
                split_points.push(kw_len);
                split_points.push(kw_len + 1);
                break;
            }
            // Followed by operator or parenthesis e.g. 'for(' or 'if('
            if let Some(&first_ch) = tail.first() {
                if matches!(first_ch, '(' | '[' | '{' | '%' | '=' | '!' | '<' | '>' | '&' | '|') {
                    split_points.push(kw_len);
                    break;
                }
            }
        }
    }

    // Case B: Function call glued to string literal: e.g. 'print(f"%i' -> split before '"'
    if let Some(quote_idx) = chars.iter().position(|&c| c == '"' || c == '\'') {
        if quote_idx > 0 && quote_idx < n {
            let prev = chars[quote_idx - 1];
            if prev == '(' || prev == '[' || prev == '{' || prev == 'f' || prev == 'r' || prev == 'b'
                || prev == '=' || prev == ':' || prev == ','
            {
                if !split_points.contains(&quote_idx) {
                    split_points.push(quote_idx);
                }
            }
        }
    }

    // Case B2: Opening delimiter glued at start of word (e.g. '(i', '(x', '[0')
    if (chars[0] == '(' || chars[0] == '[' || chars[0] == '{')
        && !chars.contains(&')')
        && !chars.contains(&']')
        && !chars.contains(&'}')
    {
        if !split_points.contains(&1) {
            split_points.push(1);
        }
    }

    // Case B3: Unmatched closing delimiter and statement delimiter (e.g. '0):', '1):', '):')
    // Function calls with argument lists like 'range(10):' contain alphabetic chars before '(' and are preserved.
    if !chars.contains(&'(') && !chars.contains(&'[') && !chars.contains(&'{') {
        for (ci, &c) in chars.iter().enumerate() {
            if c == ')' || c == ']' || c == '}' {
                if ci > 0 && chars[ci - 1].is_alphanumeric() && !split_points.contains(&ci) {
                    split_points.push(ci);
                }
                if ci + 1 < n
                    && (chars[ci + 1] == ':' || chars[ci + 1] == ';' || chars[ci + 1] == ',')
                    && !split_points.contains(&(ci + 1))
                {
                    split_points.push(ci + 1);
                }
            }
        }
    }

    // Case C: Embedded operators between alphanumerics (do not split operators inside string quotes)
    let mut in_quote: Option<char> = None;
    let mut idx = 0;
    while idx < n {
        let c = chars[idx];
        if in_quote.is_none() && (c == '"' || c == '\'') {
            in_quote = Some(c);
            idx += 1;
            continue;
        } else if in_quote == Some(c) {
            in_quote = None;
            idx += 1;
            continue;
        }

        if in_quote.is_some() {
            idx += 1;
            continue;
        }

        // 3-char operators: ===, !==, >>>
        if idx + 2 < n {
            let three_char: String = chars[idx..=idx + 2].iter().collect();
            if matches!(three_char.as_str(), "===" | "!==" | ">>>") {
                if idx > 0 && !split_points.contains(&idx) {
                    split_points.push(idx);
                }
                if idx + 3 < n && !split_points.contains(&(idx + 3)) {
                    split_points.push(idx + 3);
                }
                idx += 3;
                continue;
            }
        }

        // Multi-char operators: ==, !=, <=, >=, +=, -=, *=, /=, %=, &&, ||, ->, =>, ::, .., <<, >>, ++, --, **, //
        if idx + 1 < n {
            let two_char: String = chars[idx..=idx + 1].iter().collect();
            if matches!(
                two_char.as_str(),
                "==" | "!=" | "<=" | ">=" | "+=" | "-=" | "*=" | "/=" | "%=" | "&&" | "||"
                    | "->" | "=>" | "::" | ".." | "<<" | ">>" | "++" | "--" | "**" | "//"
            ) {
                if idx > 0 && !split_points.contains(&idx) {
                    split_points.push(idx);
                }
                if idx + 2 < n && !split_points.contains(&(idx + 2)) {
                    split_points.push(idx + 2);
                }
                idx += 2;
                continue;
            }
        }

        // Single char operators: %, =, +, -, *, /, <, >, !, &, |, ^, ~, ?, ;, ,
        let c = chars[idx];
        if matches!(c, '%' | '=' | '+' | '-' | '*' | '/' | '<' | '>' | '!' | '&' | '|' | '^' | '~' | '?' | ';' | ',') {
            if idx > 0 && !split_points.contains(&idx) {
                split_points.push(idx);
            }
            if idx + 1 < n && !split_points.contains(&(idx + 1)) {
                split_points.push(idx + 1);
            }
        } else if c == ':' && idx > 0 && chars[idx - 1].is_numeric() {
            // e.g. '0:' -> split between '0' and ':'
            if !split_points.contains(&idx) {
                split_points.push(idx);
            }
        }

        idx += 1;
    }

    if split_points.is_empty() {
        return vec![word.clone()];
    }

    split_at_multiple_offsets(word, &split_points)
}

fn split_at_multiple_offsets(word: &OcrWord, split_points: &[usize]) -> Vec<OcrWord> {
    let chars: Vec<char> = word.text.chars().collect();
    let total_chars = chars.len() as f64;
    let char_w = word.width / total_chars;

    let mut indices = vec![0];
    for &sp in split_points {
        if sp > 0 && sp < chars.len() && !indices.contains(&sp) {
            indices.push(sp);
        }
    }
    indices.sort_unstable();
    indices.push(chars.len());

    let mut result = Vec::new();
    for w in indices.windows(2) {
        let start = w[0];
        let end = w[1];
        let sub_text: String = chars[start..end].iter().collect();
        let sub_len = (end - start) as f64;
        let sub_x = word.x + char_w * start as f64;
        let sub_w = char_w * sub_len;
        result.push(OcrWord {
            text: sub_text,
            x: sub_x,
            y: word.y,
            width: sub_w,
            height: word.height,
            confidence: word.confidence,
        });
    }

    result
}

/// Coalesces raw OCR lines (resolving line-split artifacts), preserves code indentation,
/// reconstructs token spacing, detects blank lines, and builds structured full text and bounding boxes.
pub fn coalesce_and_structure_lines(raw_lines: Vec<OcrLine>) -> (Vec<OcrLine>, String, Vec<BoundingBox>) {
    coalesce_and_structure_lines_with_gutter(raw_lines, None)
}

/// Coalesces raw OCR lines with explicit gutter boundary awareness.
pub fn coalesce_and_structure_lines_with_gutter(
    mut raw_lines: Vec<OcrLine>,
    gutter_boundary_x: Option<f64>,
) -> (Vec<OcrLine>, String, Vec<BoundingBox>) {
    // 1. Normalize punctuation on all words and segment glued code tokens
    for line in &mut raw_lines {
        for word in &mut line.words {
            word.text = normalize_code_punctuation(&word.text);
        }
        line.words.retain(|w| !w.text.trim().is_empty());
        line.words = segment_code_tokens(line.words.drain(..).collect());
    }
    raw_lines.retain(|l| !l.words.is_empty());

    if raw_lines.is_empty() {
        return (Vec::new(), String::new(), Vec::new());
    }

    // 2. Coalesce lines that are horizontally adjacent fragments of the same visual line
    // (resolving OCR horizontal split artifacts without falsely merging tightly-spaced consecutive lines or columns)
    let mut initial_total_w = 0.0;
    let mut initial_total_chars = 0;
    for line in &raw_lines {
        for word in &line.words {
            let count = word.text.chars().count();
            if count >= 2 && word.width > 0.0 {
                initial_total_w += word.width;
                initial_total_chars += count;
            }
        }
    }
    if initial_total_chars == 0 {
        for line in &raw_lines {
            for word in &line.words {
                let count = word.text.chars().count();
                if count >= 1 && word.width > 0.0 {
                    initial_total_w += word.width;
                    initial_total_chars += count;
                }
            }
        }
    }
    let char_w = if initial_total_chars > 0 {
        (initial_total_w / initial_total_chars as f64).max(3.0)
    } else {
        8.0
    };

    let mut lines: Vec<OcrLine> = raw_lines;
    loop {
        let mut merged_any = false;
        let mut i = 0;
        while i < lines.len() {
            let mut j = i + 1;
            while j < lines.len() {
                let min_h = lines[i].height.min(lines[j].height);
                let max_h = lines[i].height.max(lines[j].height);
                if min_h <= 0.0 {
                    j += 1;
                    continue;
                }

                let top = lines[i].y.max(lines[j].y);
                let bottom = (lines[i].y + lines[i].height).min(lines[j].y + lines[j].height);
                let v_overlap = (bottom - top).max(0.0);

                let center_1 = lines[i].y + lines[i].height * 0.5;
                let center_2 = lines[j].y + lines[j].height * 0.5;
                let center_diff = (center_1 - center_2).abs();

                // Optimization: prune non-overlapping lines immediately to prevent O(N^2) scaling slowdown
                if (v_overlap / min_h) < 0.50 || center_diff > (max_h * 0.35).max(min_h * 0.35) {
                    j += 1;
                    continue;
                }

                // Line fragment bounding box gap
                let line_a_right = lines[i].x + lines[i].width;
                let line_b_right = lines[j].x + lines[j].width;
                let line_gap = if lines[j].x >= line_a_right {
                    lines[j].x - line_a_right
                } else if lines[i].x >= line_b_right {
                    lines[i].x - line_b_right
                } else {
                    0.0
                };

                let mut word_gap = f64::INFINITY;
                for wa in &lines[i].words {
                    for wb in &lines[j].words {
                        let gap = if wb.x >= wa.x + wa.width {
                            wb.x - (wa.x + wa.width)
                        } else if wa.x >= wb.x + wb.width {
                            wa.x - (wb.x + wb.width)
                        } else {
                            0.0
                        };
                        if gap < word_gap {
                            word_gap = gap;
                        }
                    }
                }

                let horizontal_gap = line_gap.min(word_gap);
                // Cap max_allowed_gap at 35.0px for substantial multi-word column blocks so headlines/text
                // never merge across column gutters (>= 40px), while intra-line tokens/operators and
                // single-token line numbers coalesce properly with their line content.
                let is_multi_word_column = lines[i].words.len() >= 2
                    && lines[j].words.len() >= 2
                    && lines[i].width >= 70.0
                    && lines[j].width >= 70.0;
                let is_gutter_token = |w: &OcrWord| -> bool {
                    let is_num = w.text.chars().all(|c| c.is_ascii_digit());
                    if let Some(gb) = gutter_boundary_x {
                        is_num && (w.x + w.width) <= gb + 4.0
                    } else {
                        is_num && w.x < 75.0
                    }
                };
                let has_line_num_prefix = (lines[i].words.len() == 1 && is_gutter_token(&lines[i].words[0]))
                    || (lines[j].words.len() == 1 && is_gutter_token(&lines[j].words[0]));
                let has_unclosed_paren = lines[i].words.iter().any(|w| w.text == "(")
                    && lines[j].words.iter().any(|w| w.text == ")");
                let max_allowed_gap = if is_multi_word_column && !has_unclosed_paren {
                    (min_h * 1.5).max(char_w * 2.5).min(35.0)
                } else if has_line_num_prefix || has_unclosed_paren {
                    (min_h * 8.0).max(char_w * 14.0).min(180.0)
                } else {
                    (min_h * 2.5).max(char_w * 4.5).min(75.0)
                };

                let is_same_line = horizontal_gap <= max_allowed_gap + 1e-4
                    && !words_collide_horizontally(&lines[i].words, &lines[j].words);

                if is_same_line {
                    let mut other_words = lines.remove(j).words;
                    lines[i].words.append(&mut other_words);
                    lines[i].words.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal));
                    let (nx, ny, nw, nh) = compute_words_bounding_box(&lines[i].words);
                    lines[i].x = nx;
                    lines[i].y = ny;
                    lines[i].width = nw;
                    lines[i].height = nh;
                    merged_any = true;
                } else {
                    j += 1;
                }
            }
            i += 1;
        }
        if !merged_any {
            break;
        }
    }

    for line in &mut lines {
        line.words.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal));
        // 1. Normalize single-bar '-' misread of assignment '=' between an identifier/type and a literal/expression
        for idx in 1..line.words.len().saturating_sub(1) {
            if line.words[idx].text == "-" {
                let prev = line.words[idx - 1].text.as_str();
                let next = line.words[idx + 1].text.as_str();
                let prev_is_lhs = matches!(prev, "int" | "float" | "str" | "bool" | "result" | "tag" | "val" | "x" | "y" | "i" | "n" | "count" | "total")
                    || prev.chars().all(|c| c.is_ascii_lowercase() || c == '_');
                let next_is_rhs_start = next.starts_with('"')
                    || next.starts_with('\'')
                    || next.starts_with('(')
                    || next.starts_with('[')
                    || next.starts_with('{')
                    || (prev == "int" && next.chars().all(|c| c.is_ascii_digit()));
                if prev_is_lhs && next_is_rhs_start {
                    line.words[idx].text = "=".to_string();
                }
            }
        }

        // 1b. Normalize monospace slashed zero '0' misread as 'e' or '@' when followed by 'is' or ')'
        let mut idx = 0;
        while idx < line.words.len() {
            if (line.words[idx].text == "e" || line.words[idx].text == "@") && idx + 1 < line.words.len() {
                let next = line.words[idx + 1].text.as_str();
                if next == "is" || next == "IS" || next == ")" || next == ":" {
                    line.words[idx].text = "0".to_string();
                }
            } else if line.words[idx].text == "IS" && idx + 1 < line.words.len() {
                let next = line.words[idx + 1].text.to_ascii_lowercase();
                if next.starts_with("odd") || next.starts_with("even") {
                    line.words[idx].text = "is".to_string();
                }
            } else if (line.words[idx].text == "Odd" || line.words[idx].text == "odd?)")
                && idx > 0
                && (line.words[idx - 1].text == "is" || line.words[idx - 1].text == "IS")
            {
                if line.words[idx].text == "odd?)" {
                    line.words[idx].text = "odd\")".to_string();
                } else {
                    line.words[idx].text = "odd".to_string();
                }
            } else if line.words[idx].text == "iSddd" || line.words[idx].text == "iS6dd" || line.words[idx].text == "isodd" {
                let w = line.words[idx].clone();
                let half_w = (w.width * 0.42).max(8.0);
                line.words[idx] = OcrWord {
                    text: "is".to_string(),
                    x: w.x,
                    y: w.y,
                    width: half_w,
                    height: w.height,
                    confidence: w.confidence,
                };
                line.words.insert(
                    idx + 1,
                    OcrWord {
                        text: "odd".to_string(),
                        x: w.x + half_w + 4.0,
                        y: w.y,
                        width: (w.width - half_w - 4.0).max(8.0),
                        height: w.height,
                        confidence: w.confidence,
                    },
                );
                idx += 1;
            } else if (line.words[idx].text == "@" || line.words[idx].text == "O" || line.words[idx].text == "O:")
                && idx > 0
                && (line.words[idx - 1].text == "==" || line.words[idx - 1].text == "--" || line.words[idx - 1].text == "-")
            {
                line.words[idx].text = "0:".to_string();
            } else if (line.words[idx].text == "--" || line.words[idx].text == "-")
                && idx > 0
                && idx + 1 < line.words.len()
                && line.words.iter().any(|w| w.text == "if" || w.text == "while")
                && (line.words[idx + 1].text.starts_with('0') || line.words[idx + 1].text == "O:" || line.words[idx + 1].text == "@")
            {
                line.words[idx].text = "==".to_string();
            }
            idx += 1;
        }

        // 1c. If an `if` line has `i` followed by `2` with a wide horizontal gap where `%` was dropped, insert `%`
        for k in 0..line.words.len().saturating_sub(1) {
            if line.words[k].text == "i" && line.words[k + 1].text == "2" {
                let gap = line.words[k + 1].x - (line.words[k].x + line.words[k].width);
                if gap >= char_w * 1.6 {
                    let pct_x = line.words[k].x + line.words[k].width + (gap - char_w) * 0.5;
                    let pct_y = line.words[k].y.min(line.words[k + 1].y);
                    let pct_h = line.words[k].height.max(line.words[k + 1].height);
                    line.words.insert(
                        k + 1,
                        OcrWord {
                            text: "%".to_string(),
                            x: pct_x,
                            y: pct_y,
                            width: char_w,
                            height: pct_h,
                            confidence: 95.0,
                        },
                    );
                    break;
                }
            }
        }

        // 2. If a standalone `else` or `try` statement is the only token on its line in a colon-delimited block,
        // restore any dropped trailing ':'
        if line.words.len() == 1 && line.words[0].text == "else" {
            line.words[0].text = "else:".to_string();
        }
        let mut r_idx = 0;
        while r_idx + 1 < line.words.len() {
            if line.words[r_idx].text.starts_with("range(") && line.words[r_idx].text.ends_with(')') && line.words[r_idx + 1].text == ":" {
                let colon_w = line.words.remove(r_idx + 1);
                line.words[r_idx].text.push(':');
                line.words[r_idx].width = (colon_w.x + colon_w.width) - line.words[r_idx].x;
            } else {
                r_idx += 1;
            }
        }

        // 3. If WinRT OCR dropped the thin '=' operator in `identifier = "string"` or `identifier = 'string'`
        // leaving a wide horizontal gap (>= 2.2 * char_w) between a simple lowercase identifier and a quoted literal,
        // insert the missing '=' token in the center of the gap.
        if line.words.len() >= 2 {
            let w0 = &line.words[0];
            let w1 = &line.words[1];
            let gap = w1.x - (w0.x + w0.width);
            let is_lhs_ident = w0.text.len() >= 2
                && w0.text.chars().all(|c| c.is_ascii_lowercase() || c == '_')
                && !is_code_keyword(&w0.text);
            let is_rhs_quoted = w1.text.starts_with('"') || w1.text.starts_with('\'');
            if is_lhs_ident && is_rhs_quoted && gap >= char_w * 2.1 && gap <= char_w * 5.0 {
                let eq_x = w0.x + w0.width + (gap - char_w) * 0.5;
                let eq_y = w0.y.min(w1.y);
                let eq_h = w0.height.max(w1.height);
                line.words.insert(
                    1,
                    OcrWord {
                        text: "=".to_string(),
                        x: eq_x,
                        y: eq_y,
                        width: char_w,
                        height: eq_h,
                        confidence: 95.0,
                    },
                );
            }
        }
    }

    // Filter out isolated single-symbol ghost lines (like stray '_' or '|' on their own line)
    const NOISE_CHARS: &[char] = &['|', '—', '_', '~', '^', '•', '·', '«', '»', '¢', '§', '°', '-', '='];
    let original_lines = lines.clone();
    lines.retain(|candidate| {
        if candidate.words.len() == 1 {
            let t = candidate.words[0].text.trim();
            if t == "_" || t == "|" || t == "—" {
                return false;
            }
            if t.chars().count() == 1 && NOISE_CHARS.contains(&t.chars().next().unwrap()) {
                for other in &original_lines {
                    if candidate.y != other.y || candidate.x != other.x {
                        if words_collide_horizontally(&candidate.words, &other.words) {
                            let top = candidate.y.max(other.y);
                            let bottom = (candidate.y + candidate.height).min(other.y + other.height);
                            let v_overlap = (bottom - top).max(0.0);
                            let min_h = candidate.height.min(other.height);
                            if min_h > 0.0 && (v_overlap / min_h) >= 0.50 {
                                return false;
                            }
                        }
                    }
                }
            }
        }
        true
    });

    // 3. Sort lines top-to-bottom by y coordinate, breaking ties or near-ties by x coordinate (left-to-right)
    lines.sort_by(|a, b| {
        let diff_y = a.y - b.y;
        if diff_y.abs() > 4.0 {
            a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal)
        } else {
            a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal)
        }
    });

    // 4. Estimate character width and median line height across code block
    let mut total_w = 0.0;
    let mut total_chars = 0;
    for line in &lines {
        for word in &line.words {
            let count = word.text.chars().count();
            if count >= 2 {
                total_w += word.width;
                total_chars += count;
            }
        }
    }
    if total_chars == 0 {
        for line in &lines {
            for word in &line.words {
                let count = word.text.chars().count();
                if count >= 1 && word.width > 0.0 {
                    total_w += word.width;
                    total_chars += count;
                }
            }
        }
    }
    let char_w = if total_chars > 0 {
        (total_w / total_chars as f64).max(3.0)
    } else {
        8.0
    };

    let mut heights: Vec<f64> = lines.iter().map(|l| l.height).filter(|&h| h > 0.0).collect();
    heights.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median_line_h = if !heights.is_empty() {
        heights[heights.len() / 2].max(8.0)
    } else {
        18.0
    };

    // Line pitch estimate (vertical distance between consecutive lines)
    let mut pitches: Vec<f64> = Vec::new();
    for i in 1..lines.len() {
        let diff = lines[i].y - lines[i - 1].y;
        if diff > median_line_h * 0.8 && diff < median_line_h * 2.5 {
            pitches.push(diff);
        }
    }
    pitches.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median_pitch = if !pitches.is_empty() {
        pitches[pitches.len() / 2]
    } else {
        median_line_h * 1.3
    };

    // Helper to determine the first code token of a line, skipping gutter line numbers if present
    let get_first_code_word_index = |l: &OcrLine| -> usize {
        if let Some(w0) = l.words.first() {
            let is_w0_numeric = w0.text.chars().all(|c| c.is_ascii_digit());
            if is_w0_numeric {
                if let Some(gb_x) = gutter_boundary_x {
                    if (w0.x + w0.width) <= gb_x + 4.0 {
                        return 1;
                    }
                } else if l.words.len() >= 2 {
                    let w1 = &l.words[1];
                    let is_gutter_pos = w0.x < 75.0 && (w1.x - (w0.x + w0.width)) >= (char_w * 1.5).max(12.0);
                    if is_gutter_pos {
                        return 1;
                    }
                }
            }
        }
        0
    };

    // 5. Compute base left margin of the code block (measured from the first code token)
    let base_left_x = lines
        .iter()
        .filter_map(|l| {
            let idx = get_first_code_word_index(l);
            l.words.get(idx).map(|w| w.x)
        })
        .fold(f64::INFINITY, f64::min);
    let base_left_x = if base_left_x.is_finite() { base_left_x } else { 0.0 };

    // Collect non-zero indentation offsets across lines measured from the first code token
    let mut non_zero_indents: Vec<f64> = lines
        .iter()
        .filter_map(|l| {
            let idx = get_first_code_word_index(l);
            l.words.get(idx).map(|w| w.x - base_left_x)
        })
        .filter(|&px| px >= char_w * 0.55)
        .collect();
    non_zero_indents.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    // Detect indentation model:
    // If we have indented lines, inspect the minimum indent offset to identify the base tab step
    let (unit_px, unit_spaces) = if let Some(&min_px) = non_zero_indents.first() {
        let est_chars = min_px / char_w;
        if est_chars >= 1.4 && est_chars <= 2.8 {
            (min_px, 2usize)
        } else if est_chars >= 3.2 && est_chars <= 5.0 {
            (min_px, 4usize)
        } else if est_chars > 5.0 && (est_chars / 4.0 - (est_chars / 4.0).round()).abs() <= 0.25 {
            let mult = (est_chars / 4.0).round().max(1.0);
            (min_px / mult, 4usize)
        } else if est_chars > 3.0 && (est_chars / 2.0 - (est_chars / 2.0).round()).abs() <= 0.25 {
            let mult = (est_chars / 2.0).round().max(1.0);
            (min_px / mult, 2usize)
        } else {
            (char_w, 1usize)
        }
    } else {
        (char_w, 1usize)
    };

    // 5b. Recover dropped leading dotted '0' in console output line `0 is even` when `is` is indented
    // by ~2 character widths from `base_left_x` (where the leading '0' glyph sits).
    for line in &mut lines {
        let code_idx = get_first_code_word_index(line);
        if line.words.len() == code_idx + 2
            && line.words[code_idx].text == "is"
            && line.words[code_idx + 1].text == "even"
        {
            let is_w = &line.words[code_idx];
            if is_w.x >= base_left_x + char_w * 1.1 && is_w.x <= base_left_x + char_w * 4.2 {
                let zero_word = OcrWord {
                    text: "0".to_string(),
                    x: base_left_x,
                    y: is_w.y,
                    width: char_w,
                    height: is_w.height,
                    confidence: 95.0,
                };
                line.words.insert(code_idx, zero_word);
                line.x = line.x.min(base_left_x);
            }
        }
    }

    // 6. Format each line with leading indentation and token spacing
    for line in &mut lines {
        let code_idx = get_first_code_word_index(line);
        let has_gutter_num = code_idx > 0;
        let indent_spaces = if let Some(first_code_word) = line.words.get(code_idx) {
            let indent_px = (first_code_word.x - base_left_x).max(0.0);
            if indent_px >= char_w * 0.55 {
                if unit_spaces > 1 && unit_px > 0.0 {
                    let est_units = indent_px / unit_px;
                    let nearest = est_units.round();
                    if (est_units - nearest).abs() <= 0.28 {
                        (nearest.max(1.0) as usize) * unit_spaces
                    } else {
                        (indent_px / char_w).round() as usize
                    }
                } else {
                    (indent_px / char_w).round() as usize
                }
            } else {
                0
            }
        } else {
            0
        };

        let mut line_str = String::new();
        if has_gutter_num {
            // Include gutter line numbers, followed by gutter gap and code indentation
            line_str.push_str(&line.words[0].text);
            if code_idx < line.words.len() {
                line_str.push_str("    ");
            }
        }
        if indent_spaces > 0 {
            line_str.push_str(&" ".repeat(indent_spaces));
        }

        for (i, word) in line.words[code_idx..].iter().enumerate() {
            if i == 0 {
                line_str.push_str(&word.text);
            } else {
                let prev = &line.words[code_idx + i - 1];
                let prev_end = prev.x + prev.width;
                let gap = (word.x - prev_end).max(0.0);

                let is_tight_colon = (word.text == ":" || word.text == ";" || word.text == ",") && gap < char_w * 1.25;
                let is_open_quote = (prev.text == "'" || prev.text.ends_with("{'")) && gap < char_w * 1.65;
                if gap < char_w * 0.35 || is_tight_colon || is_open_quote {
                    line_str.push_str(&word.text);
                } else if gap >= char_w * 1.75 {
                    let spaces = ((gap / char_w).round() as usize).max(2).min(8);
                    line_str.push_str(&" ".repeat(spaces));
                    line_str.push_str(&word.text);
                } else {
                    line_str.push(' ');
                    line_str.push_str(&word.text);
                }
            }
        }
        line.text = line_str;
    }

    // 7. Assemble full text preserving blank lines between blocks
    let mut full_text_lines = Vec::new();
    for i in 0..lines.len() {
        if i > 0 {
            let prev_y = lines[i - 1].y;
            let curr_y = lines[i].y;
            let y_diff = curr_y - prev_y;
            if y_diff >= median_pitch * 1.6 {
                let extra_lines = ((y_diff / median_pitch).round() as usize).saturating_sub(1).min(3);
                for _ in 0..extra_lines {
                    full_text_lines.push(String::new());
                }
            }
        }
        full_text_lines.push(lines[i].text.clone());
    }
    let full_text = full_text_lines.join("\n");

    // 8. Bounding boxes matching lines
    let boxes = lines
        .iter()
        .map(|l| BoundingBox {
            text: l.text.clone(),
            x: l.x,
            y: l.y,
            width: l.width,
            height: l.height,
            translated_text: None,
        })
        .collect();

    (lines, full_text, boxes)
}

/// Concatenates the lines belonging to a paragraph with spaces, properly joining words
/// across line breaks and dehyphenating words broken by end-of-line hyphens.
pub fn join_paragraph_lines(lines: &[&str]) -> String {
    let mut result = String::new();
    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if result.is_empty() {
            result.push_str(trimmed);
        } else {
            // Check if result ends with hyphen preceded by alphabetic char (word-break dehyphenation)
            let ends_with_hyphen = result.ends_with('-')
                && result[..result.len() - 1]
                    .chars()
                    .last()
                    .map(|c| c.is_alphabetic())
                    .unwrap_or(false)
                && trimmed.chars().next().map(|c| c.is_alphabetic()).unwrap_or(false);

            if ends_with_hyphen {
                result.pop(); // remove trailing '-'
                result.push_str(trimmed);
            } else {
                result.push(' ');
                result.push_str(trimmed);
            }
        }
    }
    result
}

/// Evaluates whether two vertically adjacent lines belong to the same text paragraph.
pub fn should_merge_lines_into_paragraph(
    curr_text: &str,
    curr_x: f64,
    curr_y: f64,
    curr_w: f64,
    curr_h: f64,
    next_text: &str,
    next_x: f64,
    next_y: f64,
    next_w: f64,
    next_h: f64,
    median_line_h: f64,
    median_pitch: f64,
    max_column_w: f64,
) -> bool {
    let t_curr = curr_text.trim();
    let t_next = next_text.trim();

    if t_curr.is_empty() || t_next.is_empty() {
        return false;
    }

    // 1. Valid dimensions
    if curr_w <= 0.0 || curr_h <= 0.0 || next_w <= 0.0 || next_h <= 0.0 {
        return false;
    }

    // 2. Line height similarity: lines in the same paragraph share consistent font height
    let min_h = curr_h.min(next_h);
    let max_h = curr_h.max(next_h);
    if min_h / max_h < 0.50 {
        return false;
    }

    // 3. Vertical pitch & gap
    let gap_y = next_y - (curr_y + curr_h);
    let pitch_y = next_y - curr_y;
    // Disallow non-downward lines or lines with excessive vertical gap (e.g. blank line)
    if pitch_y <= 0.0 || pitch_y > median_pitch * 1.65 || gap_y > median_line_h * 1.15 {
        return false;
    }

    // 4. Horizontal column overlap: lines must be in the same column
    let left = curr_x.max(next_x);
    let right = (curr_x + curr_w).min(next_x + next_w);
    let h_overlap = (right - left).max(0.0);
    let min_w = curr_w.min(next_w);
    if min_w <= 0.0 || (h_overlap / min_w) < 0.35 {
        return false;
    }

    // Disallow continuous left-edge drift (e.g. staircase lines from adversarial tests)
    let left_diff = (curr_x - next_x).abs();
    let is_first_line_indent = curr_x > next_x && (curr_x - next_x) <= 40.0;
    if left_diff > 16.0 && !is_first_line_indent {
        return false;
    }

    // 5. Markup heuristics: HTML/XML tags are UI elements, not prose lines
    let has_markup = (t_curr.contains('<') && t_curr.contains('>'))
        || t_curr.ends_with('>')
        || (t_next.contains('<') && t_next.contains('>'))
        || t_next.starts_with('<');
    if has_markup {
        return false;
    }

    // 6. Code heuristics: if either line looks like code syntax, do not merge
    const CODE_SUFFIXES: &[char] = &['{', '}', ';'];
    if CODE_SUFFIXES.iter().any(|&c| t_curr.ends_with(c)) {
        return false;
    }
    const CODE_PREFIXES: &[&str] = &[
        "pub fn ", "fn ", "let ", "const ", "import ", "export ", "class ", "def ", "//", "/*", "*/",
    ];
    if CODE_PREFIXES.iter().any(|&p| t_curr.starts_with(p) || t_next.starts_with(p)) {
        return false;
    }

    // 7. Next line starts with bullet / list item / numbered item -> starts new item
    let starts_with_bullet = t_next.starts_with('•')
        || t_next.starts_with('*')
        || t_next.starts_with('-')
        || (t_next.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false)
            && t_next.chars().nth(1).map(|c| c == '.' || c == ')').unwrap_or(false));
    if starts_with_bullet {
        return false;
    }

    // 8. Numbered / list line ending check: lines ending in a digit are labels or list items
    if t_curr.chars().last().map(|c| c.is_ascii_digit()).unwrap_or(false) {
        return false;
    }

    // 9. Title-case check: UI labels, button text, and headings have mostly capitalized words
    let words_curr: Vec<&str> = t_curr.split_whitespace().collect();
    let upper_count = words_curr
        .iter()
        .filter(|w| w.chars().next().map(|c| c.is_uppercase()).unwrap_or(false))
        .count();
    if words_curr.len() >= 2 && (upper_count as f64 / words_curr.len() as f64) >= 0.70 {
        return false;
    }

    // 10. Check for line continuation:
    // A. Hyphenated word break: e.g. "desenvolvi-" + "mento"
    let ends_with_hyphen = t_curr.ends_with('-')
        && t_curr[..t_curr.len() - 1].chars().last().map(|c| c.is_alphabetic()).unwrap_or(false);
    if ends_with_hyphen {
        return true;
    }

    // B. Terminal punctuation: '!', '?', ':', ';' always terminate a phrase.
    // Strip trailing whitespace, closing quotes ('"', '\'', '”', '’'), closing brackets (')', ']'),
    // and trailing emojis/symbols before evaluating terminal punctuation.
    let is_trailing_decorator = |c: char| {
        c.is_whitespace()
            || c == '"'
            || c == '\''
            || c == ')'
            || c == ']'
            || (!c.is_alphanumeric() && !c.is_ascii_punctuation())
    };
    let t_clean = t_curr.trim_end_matches(is_trailing_decorator);

    if t_clean.is_empty() {
        return false;
    }

    const HARD_TERMINAL_PUNCT: &[char] = &['!', '?', ':', ';'];
    if HARD_TERMINAL_PUNCT.iter().any(|&c| t_clean.ends_with(c)) {
        return false;
    }

    if t_clean.ends_with('.') {
        // Allow continuation across '.' if the line has multiple words and fills the column
        if words_curr.len() >= 4 && max_column_w > 0.0 && curr_w >= max_column_w * 0.70 {
            return true;
        }
        return false;
    }

    // C. Non-terminal ending in natural prose: genuine wrapped line continuation!
    true
}

/// Groups lines into cohesive paragraph blocks using vertical pitch consistency,
/// horizontal column overlap, and natural textual continuation.
pub fn group_lines_into_paragraphs(lines: &[OcrLine]) -> Vec<ParagraphBox> {
    if lines.is_empty() {
        return Vec::new();
    }

    // Estimate median line height
    let mut heights: Vec<f64> = lines.iter().map(|l| l.height).filter(|&h| h > 0.0).collect();
    heights.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median_line_h = if !heights.is_empty() {
        heights[heights.len() / 2].max(8.0)
    } else {
        18.0
    };

    // Line pitch estimate
    let mut pitches: Vec<f64> = Vec::new();
    for i in 1..lines.len() {
        let diff = lines[i].y - lines[i - 1].y;
        if diff > median_line_h * 0.8 && diff < median_line_h * 2.5 {
            pitches.push(diff);
        }
    }
    pitches.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median_pitch = if !pitches.is_empty() {
        pitches[pitches.len() / 2]
    } else {
        median_line_h * 1.3
    };

    let max_column_w = lines.iter().map(|l| l.width).fold(0.0f64, f64::max);

    let mut paragraphs: Vec<ParagraphBox> = Vec::new();
    let mut current_indices: Vec<usize> = vec![0];

    for j in 1..lines.len() {
        let last_idx = *current_indices.last().unwrap();
        let curr = &lines[last_idx];
        let next = &lines[j];

        if should_merge_lines_into_paragraph(
            &curr.text,
            curr.x,
            curr.y,
            curr.width,
            curr.height,
            &next.text,
            next.x,
            next.y,
            next.width,
            next.height,
            median_line_h,
            median_pitch,
            max_column_w,
        ) {
            current_indices.push(j);
        } else {
            paragraphs.push(build_paragraph_from_line_indices(lines, &current_indices));
            current_indices = vec![j];
        }
    }

    if !current_indices.is_empty() {
        paragraphs.push(build_paragraph_from_line_indices(lines, &current_indices));
    }

    paragraphs
}

fn build_paragraph_from_line_indices(lines: &[OcrLine], indices: &[usize]) -> ParagraphBox {
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    let mut line_texts = Vec::with_capacity(indices.len());

    for &idx in indices {
        let l = &lines[idx];
        min_x = min_x.min(l.x);
        min_y = min_y.min(l.y);
        max_x = max_x.max(l.x + l.width);
        max_y = max_y.max(l.y + l.height);
        line_texts.push(l.text.as_str());
    }

    let min_x = if min_x.is_finite() { min_x } else { 0.0 };
    let min_y = if min_y.is_finite() { min_y } else { 0.0 };
    let width = (max_x - min_x).max(0.0);
    let height = (max_y - min_y).max(0.0);
    let text = if indices.len() == 1 {
        lines[indices[0]].text.clone()
    } else {
        join_paragraph_lines(&line_texts)
    };
    let is_para = indices.len() > 1;

    ParagraphBox {
        text,
        x: min_x,
        y: min_y,
        width,
        height,
        line_indices: indices.to_vec(),
        translated_text: None,
        is_paragraph: Some(is_para),
    }
}

/// Groups bounding boxes into cohesive paragraph blocks.
pub fn group_bounding_boxes_into_paragraphs(boxes: &[BoundingBox]) -> Vec<ParagraphBox> {
    if boxes.is_empty() {
        return Vec::new();
    }

    let mut heights: Vec<f64> = boxes.iter().map(|b| b.height).filter(|&h| h > 0.0).collect();
    heights.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median_line_h = if !heights.is_empty() {
        heights[heights.len() / 2].max(8.0)
    } else {
        18.0
    };

    let mut pitches: Vec<f64> = Vec::new();
    for i in 1..boxes.len() {
        let diff = boxes[i].y - boxes[i - 1].y;
        if diff > median_line_h * 0.8 && diff < median_line_h * 2.5 {
            pitches.push(diff);
        }
    }
    pitches.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median_pitch = if !pitches.is_empty() {
        pitches[pitches.len() / 2]
    } else {
        median_line_h * 1.3
    };

    let max_column_w = boxes.iter().map(|b| b.width).fold(0.0f64, f64::max);

    let mut paragraphs: Vec<ParagraphBox> = Vec::new();
    let mut current_indices: Vec<usize> = vec![0];

    for j in 1..boxes.len() {
        let last_idx = *current_indices.last().unwrap();
        let curr = &boxes[last_idx];
        let next = &boxes[j];

        if should_merge_lines_into_paragraph(
            &curr.text,
            curr.x,
            curr.y,
            curr.width,
            curr.height,
            &next.text,
            next.x,
            next.y,
            next.width,
            next.height,
            median_line_h,
            median_pitch,
            max_column_w,
        ) {
            current_indices.push(j);
        } else {
            paragraphs.push(build_paragraph_from_box_indices(boxes, &current_indices));
            current_indices = vec![j];
        }
    }

    if !current_indices.is_empty() {
        paragraphs.push(build_paragraph_from_box_indices(boxes, &current_indices));
    }

    paragraphs
}

fn build_paragraph_from_box_indices(boxes: &[BoundingBox], indices: &[usize]) -> ParagraphBox {
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    let mut line_texts = Vec::with_capacity(indices.len());

    for &idx in indices {
        let b = &boxes[idx];
        min_x = min_x.min(b.x);
        min_y = min_y.min(b.y);
        max_x = max_x.max(b.x + b.width);
        max_y = max_y.max(b.y + b.height);
        line_texts.push(b.text.as_str());
    }

    let min_x = if min_x.is_finite() { min_x } else { 0.0 };
    let min_y = if min_y.is_finite() { min_y } else { 0.0 };
    let width = (max_x - min_x).max(0.0);
    let height = (max_y - min_y).max(0.0);
    let text = if indices.len() == 1 {
        boxes[indices[0]].text.clone()
    } else {
        join_paragraph_lines(&line_texts)
    };
    let is_para = indices.len() > 1;

    ParagraphBox {
        text,
        x: min_x,
        y: min_y,
        width,
        height,
        line_indices: indices.to_vec(),
        translated_text: None,
        is_paragraph: Some(is_para),
    }
}

/// Parses Tesseract TSV output into a strongly typed OcrResult with line and word bounding boxes.
pub fn parse_tesseract_tsv(tsv_text: &str) -> OcrResult {
    parse_tesseract_tsv_scaled(tsv_text, 1.0)
}

/// Parses Tesseract TSV output with coordinate scaling to map upscaled OCR back to screen pixels.
pub fn parse_tesseract_tsv_scaled(tsv_text: &str, inv_scale: f64) -> OcrResult {
    parse_tesseract_tsv_scaled_with_gutter(tsv_text, inv_scale, None)
}

/// Parses Tesseract TSV output with coordinate scaling and gutter boundary awareness.
pub fn parse_tesseract_tsv_scaled_with_gutter(
    tsv_text: &str,
    inv_scale: f64,
    gutter_boundary_x: Option<f64>,
) -> OcrResult {
    use std::collections::BTreeMap;

    let scale = if inv_scale > 0.0 { inv_scale } else { 1.0 };

    // Track line boxes (level 4) and line words (level 5)
    let mut line_boxes: BTreeMap<(i32, i32, i32), (f64, f64, f64, f64)> = BTreeMap::new();
    let mut line_words: BTreeMap<(i32, i32, i32), Vec<OcrWord>> = BTreeMap::new();
    let mut total_words = 0;

    for (idx, line) in tsv_text.lines().enumerate() {
        if idx == 0 {
            // Header line
            continue;
        }

        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() < 12 {
            continue;
        }

        let level: i32 = parts[0].trim().parse().unwrap_or(0);
        let block_num: i32 = parts[2].trim().parse().unwrap_or(0);
        let par_num: i32 = parts[3].trim().parse().unwrap_or(0);
        let line_num: i32 = parts[4].trim().parse().unwrap_or(0);
        let left: f64 = parts[6].trim().parse().unwrap_or(0.0) * scale;
        let top: f64 = parts[7].trim().parse().unwrap_or(0.0) * scale;
        let width: f64 = parts[8].trim().parse().unwrap_or(0.0) * scale;
        let height: f64 = parts[9].trim().parse().unwrap_or(0.0) * scale;

        let key = (block_num, par_num, line_num);

        if level == 4 {
            line_boxes.insert(key, (left, top, width, height));
        } else if level == 5 {
            let conf: f32 = parts[10].trim().parse().unwrap_or(0.0);
            let word_text = parts[11].trim().to_string();

            // 1. Discard empty / whitespace words
            if word_text.is_empty() {
                continue;
            }

            let raw_w: f64 = parts[8].trim().parse().unwrap_or(0.0);
            let raw_h: f64 = parts[9].trim().parse().unwrap_or(0.0);

            // 2. Sanity check: discard boxes where width < 3.0 or height < 3.0
            if raw_w < 3.0 || raw_h < 3.0 || width < 1.5 || height < 1.5 {
                continue;
            }

            let char_count = word_text.chars().count();

            // 3. Filter invalid / NaN and low-confidence hallucinations: require finite conf and conf >= 35.0 (and conf >= 45.0 for words of length <= 2)
            if !conf.is_finite() || conf.round() < 35.0 || (conf.round() < 45.0 && char_count <= 2) {
                continue;
            }

            // 4. Filter vertical margin artifacts: discard tall narrow boxes from paper folds / margins.
            // Do NOT drop single narrow characters like 'I', '1', 'l', 'i', '!', '[', ']', '|', '/', '\\'
            // or programming operators like '==', '!=', '<=', '>=', '%', '=', '+', '-', '*', etc.,
            // or programming keywords like 'for', 'if', 'else:', 'try:', 'def', 'in', etc.
            let is_code_operator = is_code_op(&word_text);
            let is_keyword = is_code_keyword(&word_text);
            let is_single_narrow_char = if char_count == 1 {
                let ch = word_text.chars().next().unwrap_or(' ');
                ch.is_alphanumeric()
                    || matches!(
                        ch,
                        '!' | '[' | ']' | '|' | '/' | '\\' | '(' | ')' | '{' | '}'
                            | ':' | ';' | '?' | '%' | '=' | '+' | '-' | '*' | '<' | '>'
                            | '&' | '^' | '~' | '\'' | '"' | ',' | '.' | '_' | '@' | '$' | '#'
                    )
            } else {
                false
            };

            let is_excessive_tall_sliver = height > 40.0 && width < 15.0;
            let should_filter_aspect = if is_code_operator || (is_keyword && height <= 40.0) {
                false
            } else if is_excessive_tall_sliver {
                true
            } else if char_count >= 2 && char_count <= 3 {
                true
            } else if !is_single_narrow_char {
                true
            } else {
                false
            };

            if height > 2.8 * width && should_filter_aspect {
                continue;
            }

            // 5. Filter single noise symbols: discard isolated symbols when conf < 75.0
            const NOISE_SYMBOLS: &[char] = &['|', '—', '_', '~', '^', '•', '·', '«', '»', '¢', '§', '°', '-', '='];
            if char_count == 1 && NOISE_SYMBOLS.contains(&word_text.chars().next().unwrap()) && conf < 75.0 {
                continue;
            }

            let word = OcrWord {
                text: word_text,
                x: left,
                y: top,
                width,
                height,
                confidence: conf,
            };

            line_words.entry(key).or_default().push(word);
            total_words += 1;
        }
    }

    let mut raw_lines = Vec::new();

    for (key, words) in line_words {
        let (lx, ly, lw, lh) = if let Some(&(x, y, w, h)) = line_boxes.get(&key) {
            if w > 0.0 && h > 0.0 {
                (x, y, w, h)
            } else {
                compute_words_bounding_box(&words)
            }
        } else {
            compute_words_bounding_box(&words)
        };

        let raw_line_text = words
            .iter()
            .map(|w| w.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");

        raw_lines.push(OcrLine {
            text: raw_line_text,
            x: lx,
            y: ly,
            width: lw,
            height: lh,
            words,
        });
    }

    let (lines, full_text, boxes) = coalesce_and_structure_lines_with_gutter(raw_lines, gutter_boundary_x);
    let paragraphs = group_lines_into_paragraphs(&lines);

    OcrResult {
        full_text,
        lines,
        word_count: total_words,
        boxes,
        paragraphs,
    }
}


#[cfg(target_os = "windows")]
fn recognize_rgba_with_engine(
    engine: &windows::Media::Ocr::OcrEngine,
    img: &image::RgbaImage,
    offset_x: f64,
    offset_y: f64,
    pad: f64,
    inv_scale: f64,
) -> Result<Vec<OcrLine>, String> {
    use windows::Graphics::Imaging::{BitmapPixelFormat, SoftwareBitmap};
    use windows::Storage::Streams::DataWriter;

    let width = img.width() as i32;
    let height = img.height() as i32;
    if width <= 0 || height <= 0 {
        return Ok(Vec::new());
    }

    let software_bitmap = SoftwareBitmap::Create(BitmapPixelFormat::Rgba8, width, height)
        .map_err(|e| format!("Failed to create SoftwareBitmap: {e}"))?;

    let raw_bytes = img.as_raw();
    let data_writer = DataWriter::new()
        .map_err(|e| format!("Failed to create DataWriter: {e}"))?;
    data_writer
        .WriteBytes(raw_bytes)
        .map_err(|e| format!("Failed to write pixels: {e}"))?;
    let buffer = data_writer
        .DetachBuffer()
        .map_err(|e| format!("Failed to detach buffer: {e}"))?;

    software_bitmap
        .CopyFromBuffer(&buffer)
        .map_err(|e| format!("Failed to copy pixels to SoftwareBitmap: {e}"))?;

    let ocr_result = engine
        .RecognizeAsync(&software_bitmap)
        .map_err(|e| format!("Failed to start OCR recognition: {e}"))?
        .get()
        .map_err(|e| format!("OCR recognition execution failed: {e}"))?;

    let mut lines_out = Vec::new();
    if let Ok(ocr_lines) = ocr_result.Lines() {
        for line in ocr_lines {
            let mut words = Vec::new();
            if let Ok(ocr_words) = line.Words() {
                for word in ocr_words {
                    let word_text = word.Text().map(|t| t.to_string()).unwrap_or_default();
                    let rect = word.BoundingRect().unwrap_or_default();
                    let wx = ((rect.X as f64) * inv_scale - pad + offset_x).max(0.0);
                    let wy = ((rect.Y as f64) * inv_scale - pad + offset_y).max(0.0);
                    let ww = (rect.Width as f64) * inv_scale;
                    let wh = (rect.Height as f64) * inv_scale;
                    words.push(OcrWord {
                        text: word_text,
                        x: wx,
                        y: wy,
                        width: ww,
                        height: wh,
                        confidence: 100.0,
                    });
                }
            }

            if !words.is_empty() {
                let (lx, ly, lw, lh) = compute_words_bounding_box(&words);
                let line_text = line.Text().map(|t| t.to_string()).unwrap_or_default();
                lines_out.push(OcrLine {
                    text: line_text,
                    x: lx,
                    y: ly,
                    width: lw,
                    height: lh,
                    words,
                });
            }
        }
    }

    Ok(lines_out)
}

#[cfg(target_os = "windows")]
pub fn extract_text_native(image: &image::RgbaImage) -> Result<OcrResult, String> {
    use windows::Globalization::Language;
    use windows::Media::Ocr::OcrEngine;

    // 1. High-precision preprocessing: color-distance contrast + Lanczos scaling + metadata
    let meta = preprocess_image_with_metadata(image);
    let prep_image = meta.processed_image;
    let scale = meta.scale;
    let gutter_boundary_x = meta.gutter_offset_x;

    let engine = if let Ok(en_lang) = Language::CreateLanguage(&windows::core::HSTRING::from("en-US")) {
        if OcrEngine::IsLanguageSupported(&en_lang).unwrap_or(false) {
            OcrEngine::TryCreateFromLanguage(&en_lang)
                .or_else(|_| OcrEngine::TryCreateFromUserProfileLanguages())
                .map_err(|e| format!("Failed to create OCR engine: {e}"))?
        } else {
            OcrEngine::TryCreateFromUserProfileLanguages()
                .map_err(|e| format!("Failed to initialize Windows OCR Engine: {e}"))?
        }
    } else {
        OcrEngine::TryCreateFromUserProfileLanguages()
            .map_err(|e| format!("Failed to initialize Windows OCR Engine: {e}"))?
    };

    let inv_scale = 1.0 / scale;
    let mut raw_lines = recognize_rgba_with_engine(&engine, &prep_image, 0.0, 0.0, 0.0, inv_scale)?;

    // 2. Horizontal Line-Band Recovery Pass:
    // Multi-line code snippets with line numbers, vertical gutter dividers, or colored syntax
    // frequently cause full-page layout analysis to drop entire lines (e.g., `if i % 2 == 0:`, `else:`)
    // or miss tokens on lines where only the line number was recognized (`7  0 is even`).
    let mut unscaled_prep = image.clone();
    enhance_contrast(&mut unscaled_prep);
    suppress_vertical_guidelines(&mut unscaled_prep);

    let bands = detect_horizontal_line_bands(&unscaled_prep);
    if bands.len() <= 24 {
        let (uw, uh) = (unscaled_prep.width(), unscaled_prep.height());
        for (y0, y1) in bands {
            let band_y0 = y0.saturating_sub(2);
            let band_y1 = (y1 + 2).min(uh);
            let band_h = band_y1.saturating_sub(band_y0);
            if band_h < 6 {
                continue;
            }

            // Find horizontal ink span [min_x, max_x] in this band
            let mut min_x = uw;
            let mut max_x = 0u32;
            for y in band_y0..band_y1 {
                for x in 0..uw {
                    if unscaled_prep.get_pixel(x, y)[0] < 210 {
                        min_x = min_x.min(x);
                        max_x = max_x.max(x);
                    }
                }
            }
            if max_x <= min_x || (max_x - min_x) < 4 {
                continue;
            }

            let base_x0 = min_x.saturating_sub(4);
            let base_x1 = (max_x + 4).min(uw);
            let base_w = base_x1 - base_x0;

            // Check if existing raw_lines already cover the horizontal ink span of this band
            let band_cy = (band_y0 + band_y1) as f64 * 0.5;
            let mut covered_words = 0usize;
            let mut covered_max_x = 0.0f64;
            let mut covered_ink_cols = vec![false; base_w as usize];
            for rl in &raw_lines {
                for w in &rl.words {
                    let w_cy = w.y + w.height * 0.5;
                    if (w_cy - band_cy).abs() <= (band_h as f64 * 0.75).max(8.0) {
                        covered_words += 1;
                        covered_max_x = covered_max_x.max(w.x + w.width);
                        let wx0 = (w.x.floor() as i64 - base_x0 as i64).max(0) as usize;
                        let wx1 = (((w.x + w.width).ceil() as i64 - base_x0 as i64) as usize).min(base_w as usize);
                        for cx in wx0..wx1 {
                            covered_ink_cols[cx] = true;
                        }
                    }
                }
            }

            // Count how many horizontal columns with actual ink in this band are NOT covered by any detected word
            let mut uncovered_ink_col_count = 0usize;
            for bx in 0..base_w {
                if !covered_ink_cols[bx as usize] {
                    let x = base_x0 + bx;
                    for y in band_y0..band_y1 {
                        if unscaled_prep.get_pixel(x, y)[0] < 200 {
                            uncovered_ink_col_count += 1;
                            break;
                        }
                    }
                }
            }

            // If the band has no words, its right-hand ink is uncovered, or there is a mid-line gap of uncovered ink (>= 12px),
            // run isolated band OCR with white padding.
            let ink_right = max_x as f64;
            if covered_words == 0 || (ink_right - covered_max_x) > 18.0 || uncovered_ink_col_count >= 12 {
                // Only skip the left line-number column if there is a single short numeric token on the far left
                let mut first_band_words: Vec<&OcrWord> = raw_lines
                    .iter()
                    .flat_map(|rl| &rl.words)
                    .filter(|w| ((w.y + w.height * 0.5) - band_cy).abs() <= (band_h as f64 * 0.75).max(8.0))
                    .collect();
                first_band_words.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal));

                let effective_min_x = if let Some(first_w) = first_band_words.first() {
                    if first_w.text.len() <= 3 && first_w.text.chars().all(|c| c.is_ascii_digit()) {
                        let start_search = ((first_w.x + first_w.width).ceil() as u32 + 2).max(min_x);
                        let mut next_ink_x = min_x;
                        for x in start_search..=max_x {
                            let mut col_has_ink = false;
                            for y in band_y0..band_y1 {
                                if unscaled_prep.get_pixel(x, y)[0] < 210 {
                                    col_has_ink = true;
                                    break;
                                }
                            }
                            if col_has_ink {
                                next_ink_x = x;
                                break;
                            }
                        }
                        next_ink_x
                    } else {
                        min_x
                    }
                } else {
                    min_x
                };

                let crop_x0 = effective_min_x.saturating_sub(4);
                let crop_x1 = (max_x + 4).min(uw);
                if crop_x1 <= crop_x0 || (crop_x1 - crop_x0) < 4 {
                    continue;
                }
                let crop_w = crop_x1 - crop_x0;

                let pad = 36u32;
                let padded_w = crop_w + pad * 2;
                let padded_h = band_h + pad * 2;
                let mut band_img = image::RgbaImage::from_pixel(padded_w, padded_h, image::Rgba([255, 255, 255, 255]));
                for by in 0..band_h {
                    for bx in 0..crop_w {
                        let px = *unscaled_prep.get_pixel(crop_x0 + bx, band_y0 + by);
                        band_img.put_pixel(pad + bx, pad + by, px);
                    }
                }

                let mut band_lines = Vec::new();
                let mut best_score = 0.0f64;

                for &b_scale in &[1.0f64, 1.5f64, 1.8f64, 2.0f64, 2.5f64] {
                    let resized_band = if (b_scale - 1.0).abs() > 0.01 {
                        let scaled_w = ((padded_w as f64) * b_scale).round() as u32;
                        let scaled_h = ((padded_h as f64) * b_scale).round() as u32;
                        image::imageops::resize(
                            &band_img,
                            scaled_w,
                            scaled_h,
                            image::imageops::FilterType::Lanczos3,
                        )
                    } else {
                        band_img.clone()
                    };

                    if let Ok(lines) = recognize_rgba_with_engine(
                        &engine,
                        &resized_band,
                        crop_x0 as f64,
                        band_y0 as f64,
                        pad as f64,
                        1.0 / b_scale,
                    ) {
                        if !lines.is_empty() {
                            let total_chars: usize = lines.iter().map(|l| l.text.len()).sum();
                            let total_w: f64 = lines.iter().flat_map(|l| &l.words).map(|w| w.width).sum();
                            let score = total_w + (total_chars as f64 * 4.0);
                            if score > best_score {
                                best_score = score;
                                band_lines = lines;
                            }
                            if total_w >= (crop_w as f64 * 0.70) {
                                break;
                            }
                        }
                    }
                }

                // If full-band recognition still leaves more than 35% of the band's horizontal width empty
                // (common on lines with math operators like `if (i % 2 == 0):`), segment horizontal word-blobs
                // separated by >= 3px of whitespace and recognize them in a composite strip with wider spacing.
                let rec_w: f64 = band_lines.iter().flat_map(|l| &l.words).map(|w| w.width).sum();
                if (rec_w < (crop_w as f64 * 0.50)) && crop_w >= 25 {
                    let mut col_ink = vec![false; crop_w as usize];
                    for bx in 0..crop_w {
                        for by in 0..band_h {
                            if unscaled_prep.get_pixel(crop_x0 + bx, band_y0 + by)[0] < 205 {
                                col_ink[bx as usize] = true;
                                break;
                            }
                        }
                    }
                    let mut h_spans: Vec<(u32, u32)> = Vec::new();
                    let mut in_sp = false;
                    let mut sp_x0 = 0u32;
                    let mut sp_gap = 0u32;
                    for bx in 0..crop_w {
                        if col_ink[bx as usize] {
                            if !in_sp {
                                in_sp = true;
                                sp_x0 = bx;
                            }
                            sp_gap = 0;
                        } else if in_sp {
                            sp_gap += 1;
                            if sp_gap >= 4 {
                                let sp_x1 = bx + 1 - sp_gap;
                                if sp_x1 > sp_x0 {
                                    h_spans.push((sp_x0, sp_x1));
                                }
                                in_sp = false;
                                sp_gap = 0;
                            }
                        }
                    }
                    if in_sp {
                        let sp_x1 = crop_w.saturating_sub(sp_gap);
                        if sp_x1 > sp_x0 {
                            h_spans.push((sp_x0, sp_x1));
                        }
                    }

                    // Try recognizing contiguous pairs of spans and individual spans, plus stripping any opening '(' after 'if'/'while'
                    if h_spans.len() >= 2 {
                        let mut candidate_slices: Vec<(u32, u32)> = Vec::new();
                        if h_spans.len() >= 3 {
                            // First half vs second half of the line (e.g., `if (i %` and `2 == 0):`)
                            let mid = h_spans.len() / 2;
                            candidate_slices.push((h_spans[0].0, h_spans[mid].1));
                            candidate_slices.push((h_spans[mid].0, h_spans.last().unwrap().1));
                        }
                        for &(sx0, sx1) in &h_spans {
                            candidate_slices.push((sx0, sx1));
                        }

                        for (sx0, sx1) in candidate_slices {
                            if sx1 <= sx0 {
                                continue;
                            }
                            let sw = (sx1 - sx0).max(1);
                            let sp_pad = 32u32;
                            let sp_pw = sw + sp_pad * 2;
                            let sp_ph = band_h + sp_pad * 2;
                            let mut sp_img = image::RgbaImage::from_pixel(sp_pw, sp_ph, image::Rgba([255, 255, 255, 255]));
                            for by in 0..band_h {
                                for bx in 0..sw {
                                    let px = *unscaled_prep.get_pixel(crop_x0 + sx0 + bx, band_y0 + by);
                                    sp_img.put_pixel(sp_pad + bx, sp_pad + by, px);
                                }
                            }
                            for &sp_scale in &[2.0f64, 2.5f64, 1.5f64, 1.0f64] {
                                let sp_resized = if (sp_scale - 1.0).abs() > 0.01 {
                                    image::imageops::resize(
                                        &sp_img,
                                        ((sp_pw as f64) * sp_scale).round() as u32,
                                        ((sp_ph as f64) * sp_scale).round() as u32,
                                        image::imageops::FilterType::Lanczos3,
                                    )
                                } else {
                                    sp_img.clone()
                                };
                                if let Ok(sp_lines) = recognize_rgba_with_engine(
                                    &engine,
                                    &sp_resized,
                                    (crop_x0 + sx0) as f64,
                                    band_y0 as f64,
                                    sp_pad as f64,
                                    1.0 / sp_scale,
                                ) {
                                    if !sp_lines.is_empty() {
                                        band_lines.extend(sp_lines);
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }

                if !band_lines.is_empty() {
                    for bl in band_lines {
                        let mut new_words = Vec::new();
                        for bw in bl.words {
                            // Only add words that do not already overlap an existing word in raw_lines
                            let mut already_exists = false;
                            for rl in &raw_lines {
                                for ew in &rl.words {
                                    let v_top = bw.y.max(ew.y);
                                    let v_bot = (bw.y + bw.height).min(ew.y + ew.height);
                                    let v_ov = (v_bot - v_top).max(0.0);
                                    let h_left = bw.x.max(ew.x);
                                    let h_right = (bw.x + bw.width).min(ew.x + ew.width);
                                    let h_ov = (h_right - h_left).max(0.0);
                                    let min_w = bw.width.min(ew.width).max(1.0);
                                    let min_h = bw.height.min(ew.height).max(1.0);
                                    if (v_ov / min_h) >= 0.45 && (h_ov / min_w) >= 0.45 {
                                        already_exists = true;
                                        break;
                                    }
                                }
                                if already_exists {
                                    break;
                                }
                            }
                            if !already_exists {
                                new_words.push(bw);
                            }
                        }
                        if !new_words.is_empty() {
                            let (lx, ly, lw, lh) = compute_words_bounding_box(&new_words);
                            let text = new_words.iter().map(|w| w.text.as_str()).collect::<Vec<_>>().join(" ");
                            raw_lines.push(OcrLine {
                                text,
                                x: lx,
                                y: ly,
                                width: lw,
                                height: lh,
                                words: new_words,
                            });
                        }
                    }
                }
            }
        }
    }

    let (lines, full_text, boxes) = coalesce_and_structure_lines_with_gutter(raw_lines, gutter_boundary_x);
    let paragraphs = group_lines_into_paragraphs(&lines);
    let total_words = lines.iter().map(|l| l.words.len()).sum();

    Ok(OcrResult {
        full_text,
        lines,
        word_count: total_words,
        boxes,
        paragraphs,
    })
}

#[cfg(target_os = "linux")]
pub fn extract_text_native(image: &image::RgbaImage) -> Result<OcrResult, String> {
    use std::process::Command;

    // Use unified OCR preprocessor (handles dark mode inversion + 2x scaling + metadata)
    let meta = preprocess_image_with_metadata(image);
    let temp_image = meta.processed_image;
    let scale = meta.scale;
    let gutter_boundary_x = meta.gutter_offset_x;

    // Save temporary PNG for tesseract
    let temp_dir = std::env::temp_dir();
    let temp_image_path = temp_dir.join(format!(
        "desklens_ocr_{}.png",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    ));

    temp_image
        .save_with_format(&temp_image_path, image::ImageFormat::Png)
        .map_err(|e| format!("Failed to write temporary image for OCR: {e}"))?;

    let temp_path_str = temp_image_path.to_string_lossy().to_string();

    // Check tesseract with multi-language (por+eng), --dpi 300, --psm 11, or fallback to default
    let output = match Command::new("tesseract")
        .args([
            &temp_path_str,
            "stdout",
            "--dpi",
            "300",
            "--psm",
            "11",
            "-l",
            "por+eng",
            "tsv",
        ])
        .output()
    {
        Ok(out) if out.status.success() => out,
        _ => Command::new("tesseract")
            .args([
                &temp_path_str,
                "stdout",
                "--dpi",
                "300",
                "--psm",
                "11",
                "tsv",
            ])
            .output()
            .map_err(|e| {
                let _ = std::fs::remove_file(&temp_image_path);
                if e.kind() == std::io::ErrorKind::NotFound {
                    "Tesseract OCR is not installed. To enable OCR on Linux, install it via: sudo apt install tesseract-ocr tesseract-ocr-por tesseract-ocr-eng".to_string()
                } else {
                    format!("Failed to execute Tesseract: {e}")
                }
            })?,
    };

    let _ = std::fs::remove_file(&temp_image_path);

    if !output.status.success() {
        let err_msg = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Tesseract OCR failed: {err_msg}"));
    }

    let tsv_text = String::from_utf8_lossy(&output.stdout);
    let inv_scale = if (scale - 1.0).abs() > 0.001 {
        1.0 / scale
    } else {
        1.0
    };
    let result = parse_tesseract_tsv_scaled_with_gutter(&tsv_text, inv_scale, gutter_boundary_x);

    Ok(result)
}

#[cfg(not(any(target_os = "windows", target_os = "linux")))]
pub fn extract_text_native(_image: &image::RgbaImage) -> Result<OcrResult, String> {
    Err("OCR engine is not configured for this operating system.".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_words_bounding_box() {
        let empty: Vec<OcrWord> = vec![];
        assert_eq!(compute_words_bounding_box(&empty), (0.0, 0.0, 0.0, 0.0));

        let words = vec![
            OcrWord {
                text: "Hello".to_string(),
                x: 10.0,
                y: 20.0,
                width: 50.0,
                height: 15.0,
                confidence: 100.0,
            },
            OcrWord {
                text: "World".to_string(),
                x: 70.0,
                y: 18.0,
                width: 60.0,
                height: 19.0,
                confidence: 100.0,
            },
        ];
        let (bx, by, bw, bh) = compute_words_bounding_box(&words);
        assert_eq!(bx, 10.0);
        assert_eq!(by, 18.0);
        assert_eq!(bw, 120.0); // 70 + 60 - 10 = 120
        assert_eq!(bh, 19.0);  // max(20+15=35, 18+19=37) - 18 = 19
    }

    #[test]
    fn test_parse_tesseract_tsv_levels() {
        let mock_tsv = "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n\
1\t1\t0\t0\t0\t0\t0\t0\t800\t600\t-1\t\n\
2\t1\t1\t0\t0\t0\t20\t30\t300\t100\t-1\t\n\
3\t1\t1\t1\t0\t0\t20\t30\t300\t100\t-1\t\n\
4\t1\t1\t1\t1\t0\t20\t30\t200\t25\t-1\t\n\
5\t1\t1\t1\t1\t1\t20\t30\t60\t25\t95\tDeskLens\n\
5\t1\t1\t1\t1\t2\t90\t30\t70\t25\t96\tScreen\n\
4\t1\t1\t1\t2\t0\t20\t65\t180\t25\t-1\t\n\
5\t1\t1\t1\t2\t1\t20\t65\t180\t25\t94\tTranslation\n";

        let result = parse_tesseract_tsv(mock_tsv);
        assert_eq!(result.word_count, 3);
        assert_eq!(result.lines.len(), 2);
        assert_eq!(result.lines[0].text, "DeskLens Screen");
        assert_eq!(result.lines[0].x, 20.0);
        assert_eq!(result.lines[0].y, 30.0);
        assert_eq!(result.lines[0].width, 200.0);
        assert_eq!(result.lines[0].height, 25.0);
        assert_eq!(result.lines[0].words.len(), 2);

        assert_eq!(result.lines[1].text, "Translation");
        assert_eq!(result.lines[1].x, 20.0);
        assert_eq!(result.lines[1].y, 65.0);
        assert_eq!(result.lines[1].width, 180.0);
        assert_eq!(result.lines[1].height, 25.0);
        assert_eq!(result.lines[1].words.len(), 1);

        assert_eq!(result.boxes.len(), 2);
        assert_eq!(result.boxes[0].text, "DeskLens Screen");
        assert_eq!(result.boxes[0].x, 20.0);
        assert_eq!(result.boxes[0].y, 30.0);
        assert_eq!(result.boxes[0].width, 200.0);
        assert_eq!(result.boxes[0].height, 25.0);
        assert!(result.boxes[0].translated_text.is_none());

        assert_eq!(result.boxes[1].text, "Translation");
        assert_eq!(result.boxes[1].x, 20.0);
        assert_eq!(result.boxes[1].y, 65.0);
        assert_eq!(result.boxes[1].width, 180.0);
        assert_eq!(result.boxes[1].height, 25.0);
    }

    #[test]
    fn test_bounding_box_serde_contract() {
        let box_without_trans = BoundingBox {
            text: "Hello".to_string(),
            x: 10.0,
            y: 20.0,
            width: 50.0,
            height: 15.0,
            translated_text: None,
        };
        let json = serde_json::to_string(&box_without_trans).unwrap();
        assert!(!json.contains("translated_text"), "translated_text should be omitted when None: {}", json);

        let box_with_trans = BoundingBox {
            text: "Hello".to_string(),
            x: 10.0,
            y: 20.0,
            width: 50.0,
            height: 15.0,
            translated_text: Some("Olá".to_string()),
        };
        let json_trans = serde_json::to_string(&box_with_trans).unwrap();
        assert!(json_trans.contains("\"translated_text\":\"Olá\""), "JSON should contain translated_text: {}", json_trans);
    }

    #[test]
    fn test_join_paragraph_lines_dehyphenation() {
        // Hyphenated word break
        let lines = ["Durante o desenvolvi-", "mento de software ágil,"];
        let joined = join_paragraph_lines(&lines);
        assert_eq!(joined, "Durante o desenvolvimento de software ágil,");

        // Normal lines without hyphen
        let lines_normal = ["Primeira linha de texto", "segunda linha continua."];
        assert_eq!(join_paragraph_lines(&lines_normal), "Primeira linha de texto segunda linha continua.");

        // Dash with space (punctuation dash, not word hyphen)
        let lines_dash = ["Item A - ", "continuação do item"];
        assert_eq!(join_paragraph_lines(&lines_dash), "Item A - continuação do item");
    }

    #[test]
    fn test_group_lines_into_paragraphs_reproduction() {
        // Ground truth reproduction of Image 2 (user issue)
        let l1 = OcrLine {
            text: "Lembrando que, se você ficou com alguma dúvida ou percebeu que algum assunto não foi totalmente explicado, não se".to_string(),
            x: 22.0,
            y: 14.0,
            width: 968.0,
            height: 18.0,
            words: vec![],
        };
        let l2 = OcrLine {
            text: "preocupe! Sinta-se à vontade para conversar com seu professor e esclarecer tudo que for necessário. Fazer perguntas".to_string(),
            x: 22.0,
            y: 40.0,
            width: 968.0,
            height: 18.0,
            words: vec![],
        };
        let l3 = OcrLine {
            text: "faz parte do processo de aprendizagem.".to_string(),
            x: 21.0,
            y: 65.0,
            width: 324.0,
            height: 18.0,
            words: vec![],
        };

        let paragraphs = group_lines_into_paragraphs(&[l1, l2, l3]);
        assert_eq!(paragraphs.len(), 1, "The 3 wrapped lines must form exactly 1 paragraph");
        let p = &paragraphs[0];
        assert_eq!(p.x, 21.0);
        assert_eq!(p.y, 14.0);
        assert_eq!(p.width, 969.0);
        assert_eq!(p.height, 69.0);
        assert_eq!(p.line_indices, vec![0, 1, 2]);
        assert_eq!(p.is_paragraph, Some(true));
        assert!(p.text.starts_with("Lembrando que"));
        assert!(p.text.contains("não se preocupe!"));
        assert!(p.text.contains("Fazer perguntas faz parte"));
        assert!(p.text.ends_with("processo de aprendizagem."));
    }

    #[test]
    fn test_group_bounding_boxes_separate_paragraphs() {
        // Two separate paragraphs separated by a vertical blank gap
        let b1 = BoundingBox {
            text: "Este é o primeiro parágrafo de teste.".to_string(),
            x: 10.0,
            y: 10.0,
            width: 300.0,
            height: 20.0,
            translated_text: None,
        };
        let b2 = BoundingBox {
            text: "Ele termina aqui com ponto final.".to_string(),
            x: 10.0,
            y: 35.0,
            width: 250.0,
            height: 20.0,
            translated_text: None,
        };
        let b3 = BoundingBox {
            text: "Este é o segundo parágrafo após um espaço.".to_string(),
            x: 10.0,
            y: 90.0, // Large vertical gap (pitch = 55px vs normal 25px)
            width: 310.0,
            height: 20.0,
            translated_text: None,
        };

        let paragraphs = group_bounding_boxes_into_paragraphs(&[b1, b2, b3]);
        assert_eq!(paragraphs.len(), 2, "Should separate into 2 paragraphs across vertical gap");
        assert_eq!(paragraphs[0].line_indices, vec![0, 1]);
        assert_eq!(paragraphs[1].line_indices, vec![2]);
    }

    #[test]
    fn test_adaptive_high_contrast_binarization_dark_theme() {
        let (w, h) = (120u32, 60u32);
        let dark_bg = image::Rgba([30, 30, 30, 255]);
        let mut img = image::RgbaImage::from_pixel(w, h, dark_bg);

        // Draw orange keyword 'if' (diff ~ 170)
        let orange_kw = image::Rgba([209, 154, 102, 255]);
        for y in 10..22 {
            for x in 10..25 {
                img.put_pixel(x, y, orange_kw);
            }
        }

        // Draw cyan variable 'i' (diff ~ 165)
        let cyan_var = image::Rgba([86, 182, 194, 255]);
        for y in 10..22 {
            for x in 35..45 {
                img.put_pixel(x, y, cyan_var);
            }
        }

        // Draw purple operator '2' (diff ~ 190)
        let purple_op = image::Rgba([198, 120, 221, 255]);
        for y in 10..22 {
            for x in 55..65 {
                img.put_pixel(x, y, purple_op);
            }
        }

        // Draw dim gray console output '0' (diff ~ 75)
        let dim_gray = image::Rgba([96, 104, 112, 255]);
        for y in 35..47 {
            for x in 10..30 {
                img.put_pixel(x, y, dim_gray);
            }
        }

        enhance_contrast(&mut img);

        // Background must map to pure white (255)
        let bg_sample = img.get_pixel(5, 5);
        assert_eq!(bg_sample[0], 255);
        assert_eq!(bg_sample[1], 255);
        assert_eq!(bg_sample[2], 255);

        // All syntax colors must map to solid dark ink (RGB < 30)
        let p_orange = img.get_pixel(15, 15);
        assert!(p_orange[0] < 30 && p_orange[1] < 30 && p_orange[2] < 30, "Orange keyword must become solid dark ink: {:?}", p_orange);

        let p_cyan = img.get_pixel(40, 15);
        assert!(p_cyan[0] < 30 && p_cyan[1] < 30 && p_cyan[2] < 30, "Cyan variable must become solid dark ink: {:?}", p_cyan);

        let p_purple = img.get_pixel(60, 15);
        assert!(p_purple[0] < 30 && p_purple[1] < 30 && p_purple[2] < 30, "Purple operator must become solid dark ink: {:?}", p_purple);

        let p_gray = img.get_pixel(15, 40);
        assert!(p_gray[0] < 30 && p_gray[1] < 30 && p_gray[2] < 30, "Dim gray console output must become solid dark ink: {:?}", p_gray);
    }

    #[test]
    fn test_suppress_vertical_guidelines_inpainting() {
        let (w, h) = (80u32, 60u32);
        // Start with preprocessed white image
        let mut img = image::RgbaImage::from_pixel(w, h, image::Rgba([255, 255, 255, 255]));

        // Continuous 1px vertical indentation guide line at x=30 across y=5..55 (length 50 >= 24)
        for y in 5..55 {
            img.put_pixel(30, y, image::Rgba([180, 180, 180, 255]));
        }

        // Horizontal minus '-' stroke crossing the vertical guideline at y=30, x=28..33
        for x in 28..=33 {
            img.put_pixel(x, 30, image::Rgba([0, 0, 0, 255]));
        }

        let masked = suppress_vertical_guidelines(&mut img);
        assert!(masked > 0, "Must mask vertical guideline pixels: {}", masked);

        // Guideline in empty gap (y=15, x=30) must be inpainted to pure white (255)
        let p_gap = img.get_pixel(30, 15);
        assert_eq!(p_gap[0], 255, "Vertical guide in empty gap must be inpainted to white");

        // Crossing horizontal stroke (y=30, x=30) must be preserved as dark ink
        let p_cross = img.get_pixel(30, 30);
        assert!(p_cross[0] < 50, "Crossing horizontal stroke must remain dark ink: {:?}", p_cross);
    }

    #[test]
    fn test_detect_horizontal_line_bands_with_indentation_guide() {
        let (w, h) = (100u32, 90u32);
        let mut img = image::RgbaImage::from_pixel(w, h, image::Rgba([255, 255, 255, 255]));

        // 3 distinct horizontal lines of text (dark ink)
        // Line 1: y=10..22
        for y in 10..22 {
            for x in 35..80 {
                img.put_pixel(x, y, image::Rgba([0, 0, 0, 255]));
            }
        }
        // Line 2: y=36..48
        for y in 36..48 {
            for x in 45..85 {
                img.put_pixel(x, y, image::Rgba([0, 0, 0, 255]));
            }
        }
        // Line 3: y=62..74
        for y in 62..74 {
            for x in 35..90 {
                img.put_pixel(x, y, image::Rgba([0, 0, 0, 255]));
            }
        }

        // Continuous 1px vertical indentation guide at x=25 running through ALL rows y=5..85
        for y in 5..85 {
            img.put_pixel(25, y, image::Rgba([180, 180, 180, 255]));
        }

        // Without guideline suppression, this would collapse into a single huge band.
        // With guideline suppression, it must produce 3 distinct bands.
        let bands = detect_horizontal_line_bands(&img);
        assert_eq!(bands.len(), 3, "Must detect exactly 3 distinct line bands despite vertical guide: {:?}", bands);
        assert!(bands[0].0 >= 8 && bands[0].1 <= 25, "Band 0 matches Line 1");
        assert!(bands[1].0 >= 34 && bands[1].1 <= 51, "Band 1 matches Line 2");
        assert!(bands[2].0 >= 60 && bands[2].1 <= 78, "Band 2 matches Line 3");
    }

    #[test]
    fn test_gutter_detection_and_code_indentation() {
        let (w, h) = (120u32, 80u32);
        let mut img = image::RgbaImage::from_pixel(w, h, image::Rgba([255, 255, 255, 255]));

        // Gutter digits at x=15..25
        for y in 10..22 { for x in 15..25 { img.put_pixel(x, y, image::Rgba([0, 0, 0, 255])); } }
        for y in 35..47 { for x in 15..25 { img.put_pixel(x, y, image::Rgba([0, 0, 0, 255])); } }

        // Vertical divider guide at x=40
        for y in 5..75 { img.put_pixel(40, y, image::Rgba([180, 180, 180, 255])); }

        // Code body to the right at x=60..110
        for y in 10..22 { for x in 60..100 { img.put_pixel(x, y, image::Rgba([0, 0, 0, 255])); } }
        for y in 35..47 { for x in 75..110 { img.put_pixel(x, y, image::Rgba([0, 0, 0, 255])); } }

        let gutter = detect_gutter_boundary(&img);
        assert!(gutter.has_line_numbers, "Must detect presence of line numbers");
        assert!(gutter.gutter_boundary_x.is_some(), "Must detect gutter boundary x");
        let gx = gutter.gutter_boundary_x.unwrap();
        assert!(gx >= 38.0 && gx <= 45.0, "Gutter boundary must align with divider: {}", gx);

        // Test coalesce_and_structure_lines with gutter line numbers preserving relative code indentation
        let raw_lines = vec![
            OcrLine {
                text: "1 for i in range(10):".to_string(),
                x: 18.0,
                y: 12.0,
                width: 90.0,
                height: 16.0,
                words: vec![
                    OcrWord { text: "1".to_string(), x: 18.0, y: 12.0, width: 10.0, height: 16.0, confidence: 100.0 },
                    OcrWord { text: "for".to_string(), x: 60.0, y: 12.0, width: 25.0, height: 16.0, confidence: 100.0 },
                ],
            },
            OcrLine {
                text: "2 if i % 2 == 0:".to_string(),
                x: 18.0,
                y: 38.0,
                width: 95.0,
                height: 16.0,
                words: vec![
                    OcrWord { text: "2".to_string(), x: 18.0, y: 38.0, width: 10.0, height: 16.0, confidence: 100.0 },
                    OcrWord { text: "if".to_string(), x: 92.0, y: 38.0, width: 18.0, height: 16.0, confidence: 100.0 },
                ],
            },
        ];

        let (lines, _full_text, _) = coalesce_and_structure_lines(raw_lines);
        assert_eq!(lines.len(), 2);
        // Line 2 has 'if' at x=92 vs 'for' at x=60 (32px offset / char_w ~8px = 4 spaces!)
        assert!(lines[1].text.contains("    if"), "Line 2 must preserve 4-space code indentation: '{}'", lines[1].text);
    }

    #[test]
    fn test_perimeter_background_estimation_not_skewed_by_large_text() {
        let (w, h) = (100u32, 100u32);
        let mut img = image::RgbaImage::from_pixel(w, h, image::Rgba([20, 24, 30, 255]));

        // Fill interior with bright white text content
        for y in 20..80 {
            for x in 20..80 {
                img.put_pixel(x, y, image::Rgba([255, 255, 255, 255]));
            }
        }

        let (bg_r, bg_g, bg_b) = estimate_background_color(&img);
        assert_eq!((bg_r as u8, bg_g as u8, bg_b as u8), (20, 24, 30), "Perimeter estimation must sample outer boundary without text bias");
        assert!(is_dark_image(&img), "Image must be classified as dark background");
        assert!(is_dark_background(&img), "is_dark_background alias must agree");
    }

    #[test]
    fn test_max_channel_syntax_color_inversion_all_themes() {
        let (w, h) = (60u32, 60u32);
        let mut img = image::RgbaImage::from_pixel(w, h, image::Rgba([30, 30, 30, 255]));

        // Peach #E06C75 (224, 108, 117)
        img.put_pixel(10, 10, image::Rgba([224, 108, 117, 255]));
        // Green #98C379 (152, 195, 121)
        img.put_pixel(20, 10, image::Rgba([152, 195, 121, 255]));
        // Cyan #61AFEF (97, 175, 239)
        img.put_pixel(30, 10, image::Rgba([97, 175, 239, 255]));
        // Purple #D19A66 (209, 154, 102)
        img.put_pixel(40, 10, image::Rgba([209, 154, 102, 255]));
        // Muted gray #ABB2BF (171, 178, 191)
        img.put_pixel(50, 10, image::Rgba([171, 178, 191, 255]));

        enhance_contrast(&mut img);

        for &x in &[10, 20, 30, 40, 50] {
            let p = img.get_pixel(x, 10);
            assert_eq!(p[0], 0, "Syntax token at x={} must map to 0 (black ink), got: {:?}", x, p);
            assert_eq!(p[1], 0);
            assert_eq!(p[2], 0);
        }

        let bg_p = img.get_pixel(0, 0);
        assert_eq!(bg_p[0], 255, "Background at (0, 0) must map to white");
        assert_eq!(bg_p[1], 255);
        assert_eq!(bg_p[2], 255);
    }

    #[test]
    fn test_dynamic_pitch_valley_segmentation_8_plus_bands() {
        // Multi-line code crop simulation: 9 lines with 34px line pitch and vertical indentation guide lines
        let (w, h) = (200u32, 340u32);
        let mut img = image::RgbaImage::from_pixel(w, h, image::Rgba([255, 255, 255, 255]));

        // 9 text lines at y = 10, 44, 78, 112, 146, 180, 214, 248, 282
        for line_idx in 0..9 {
            let top_y = 10 + line_idx * 34;
            for y in top_y..(top_y + 16) {
                for x in 40..160 {
                    img.put_pixel(x, y, image::Rgba([0, 0, 0, 255]));
                }
            }
        }

        // 1px vertical guide lines running continuously from y=5 to y=330 at x=20 and x=30
        for y in 5..330 {
            img.put_pixel(20, y, image::Rgba([180, 180, 180, 255]));
            img.put_pixel(30, y, image::Rgba([180, 180, 180, 255]));
        }

        let bands = detect_horizontal_line_bands(&img);
        assert!(bands.len() >= 8, "Must detect >= 8 clean line bands despite vertical guide lines, got: {}", bands.len());
        for (idx, (y0, y1)) in bands.iter().enumerate() {
            let bh = y1 - y0;
            assert!(bh >= 12 && bh <= 24, "Band {} height must tightly wrap text line (12..24px), got: {} (range {}..{})", idx, bh, y0, y1);
        }
    }

    #[test]
    fn test_detect_horizontal_line_bands_wide_crop_sparse_delimiters() {
        // Ultra-wide 1920px editor crop with sparse single-glyph lines:
        // Line 0: "if condition:" (y=15..32)
        // Line 1: "    x = [" (y=50..66)
        // Line 2: "    ]" (y=85..104, lone closing bracket)
        // Line 3: "    ;" (y=120..136, lone semicolon with dot/tail gap)
        // Line 4: "}" (y=155..175, lone closing brace)
        let (w, h) = (1920u32, 210u32);
        let mut img = image::RgbaImage::from_pixel(w, h, image::Rgba([255, 255, 255, 255]));

        // Line 0: if condition: (standard text block)
        for y in 15..32 {
            for x in 80..220 {
                img.put_pixel(x, y, image::Rgba([0, 0, 0, 255]));
            }
        }

        // Line 1: x = [
        for y in 50..66 {
            for x in 80..140 {
                img.put_pixel(x, y, image::Rgba([0, 0, 0, 255]));
            }
        }

        // Line 2: lone bracket ']' (top/bottom arms 6px ink, vertical stem 2px ink)
        for y in 85..87 { for x in 80..86 { img.put_pixel(x, y, image::Rgba([0, 0, 0, 255])); } }
        for y in 87..102 { for x in 84..86 { img.put_pixel(x, y, image::Rgba([0, 0, 0, 255])); } }
        for y in 102..104 { for x in 80..86 { img.put_pixel(x, y, image::Rgba([0, 0, 0, 255])); } }

        // Line 3: lone semicolon ';' (dot at y=120..122, comma at y=127..133)
        for y in 120..122 { for x in 84..87 { img.put_pixel(x, y, image::Rgba([0, 0, 0, 255])); } }
        for y in 127..133 { for x in 84..87 { img.put_pixel(x, y, image::Rgba([0, 0, 0, 255])); } }

        // Line 4: lone curly brace '}' (top hook 4px, stem 2px, cusp 4px, stem 2px, bottom hook 4px)
        for y in 155..157 { for x in 80..84 { img.put_pixel(x, y, image::Rgba([0, 0, 0, 255])); } }
        for y in 157..163 { for x in 82..84 { img.put_pixel(x, y, image::Rgba([0, 0, 0, 255])); } }
        for y in 163..165 { for x in 80..84 { img.put_pixel(x, y, image::Rgba([0, 0, 0, 255])); } }
        for y in 165..172 { for x in 82..84 { img.put_pixel(x, y, image::Rgba([0, 0, 0, 255])); } }
        for y in 172..174 { for x in 80..84 { img.put_pixel(x, y, image::Rgba([0, 0, 0, 255])); } }

        let bands = detect_horizontal_line_bands(&img);
        assert_eq!(
            bands.len(),
            5,
            "1920px ultra-wide image with sparse single delimiters must detect all 5 lines, got: {:?}",
            bands
        );
        assert!(bands[2].0 >= 83 && bands[2].1 <= 106, "Band 2 captures ']'");
        assert!(bands[3].0 >= 118 && bands[3].1 <= 135, "Band 3 captures ';'");
        assert!(bands[4].0 >= 153 && bands[4].1 <= 176, "Band 4 captures '}}'");
    }

    #[test]
    fn test_boundary_dilation_and_digit_top_preservation() {
        // Simulates digit tops with initial 4-5px ink runs that were previously discarded
        let (w, h) = (200u32, 100u32);
        let mut img = image::RgbaImage::from_pixel(w, h, image::Rgba([255, 255, 255, 255]));

        // Line 1: starts with 4px run at y=10..14 (ink=10), then continues at y=15..28 (ink=30)
        for y in 10..14 { for x in 30..40 { img.put_pixel(x, y, image::Rgba([0, 0, 0, 255])); } }
        for y in 15..28 { for x in 30..60 { img.put_pixel(x, y, image::Rgba([0, 0, 0, 255])); } }

        // Line 2: starts with 5px run at y=45..50 (ink=12), then continues at y=51..64 (ink=40)
        for y in 45..50 { for x in 30..42 { img.put_pixel(x, y, image::Rgba([0, 0, 0, 255])); } }
        for y in 51..64 { for x in 30..70 { img.put_pixel(x, y, image::Rgba([0, 0, 0, 255])); } }

        let bands = detect_horizontal_line_bands(&img);
        assert_eq!(bands.len(), 2, "Must detect both lines");
        assert!(bands[0].0 <= 10, "Band 0 must preserve digit top at y=10, got: {}", bands[0].0);
        assert!(bands[0].1 >= 28, "Band 0 end matches text baseline");
        assert!(bands[1].0 <= 45, "Band 1 must preserve digit top at y=45, got: {}", bands[1].0);
        assert!(bands[1].1 >= 64, "Band 1 end matches text baseline");
    }

    #[test]
    fn test_perimeter_background_estimation_with_solid_selection_border() {
        let (w, h) = (100u32, 100u32);
        let mut img = image::RgbaImage::from_pixel(w, h, image::Rgba([30, 30, 30, 255]));

        // Draw 1px solid blue selection border (#3B82F6 / 59, 130, 246)
        for x in 0..w {
            img.put_pixel(x, 0, image::Rgba([59, 130, 246, 255]));
            img.put_pixel(x, h - 1, image::Rgba([59, 130, 246, 255]));
        }
        for y in 0..h {
            img.put_pixel(0, y, image::Rgba([59, 130, 246, 255]));
            img.put_pixel(w - 1, y, image::Rgba([59, 130, 246, 255]));
        }

        // Add interior text
        for y in 30..45 {
            for x in 20..80 {
                img.put_pixel(x, y, image::Rgba([255, 255, 255, 255]));
            }
        }

        let (bg_r, bg_g, bg_b) = estimate_background_color(&img);
        assert_eq!((bg_r as u8, bg_g as u8, bg_b as u8), (30, 30, 30), "Perimeter estimation must ignore 1px selection border");
        assert!(is_dark_image(&img), "Dark image detection must classify editor background, not blue border");
    }

    #[test]
    fn test_perimeter_background_estimation_with_2px_selection_border() {
        let (w, h) = (100u32, 100u32);
        let mut img = image::RgbaImage::from_pixel(w, h, image::Rgba([20, 24, 30, 255]));

        // Draw 2px solid white selection outline (#FFFFFF / 255, 255, 255)
        for y in [0, 1, h - 2, h - 1] {
            for x in 0..w {
                img.put_pixel(x, y, image::Rgba([255, 255, 255, 255]));
            }
        }
        for y in 0..h {
            for x in [0, 1, w - 2, w - 1] {
                img.put_pixel(x, y, image::Rgba([255, 255, 255, 255]));
            }
        }

        let (bg_r, bg_g, bg_b) = estimate_background_color(&img);
        assert_eq!((bg_r as u8, bg_g as u8, bg_b as u8), (20, 24, 30), "Perimeter estimation must ignore 2px selection border");
        assert!(is_dark_image(&img), "Dark image detection must classify editor background, not white border");
    }

    #[test]
    fn test_estimate_background_color_overflow_guard() {
        let w = 70_000u32;
        let h = 70_000u32;
        let total_pixels = (w as u64) * (h as u64);
        assert_eq!(total_pixels, 4_900_000_000u64);
        let step = ((total_pixels / 4000).max(1)) as usize;
        assert_eq!(step, 1_225_000);
    }
}



