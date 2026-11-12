//! Integration with Google Lens Web OCR endpoint (Chromium protocol).
use std::time::Duration;
use image::RgbaImage;
use crate::ocr::{
    group_lines_into_paragraphs, BoundingBox, OcrLine, OcrResult, OcrWord,
};

struct LensInternalWord {
    text: String,
    separator: String,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

struct LensInternalLine {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    words: Vec<LensInternalWord>,
}

/// Extracts text from an image crop using Google Lens Web API.
/// Preserves native Google Lens word separators and spacing.
pub fn extract_text_lens(image: &RgbaImage, mode: &str) -> Result<OcrResult, String> {
    let _ = mode;
    let (orig_w, orig_h) = (image.width() as f64, image.height() as f64);
    if orig_w == 0.0 || orig_h == 0.0 {
        return Ok(OcrResult {
            full_text: String::new(),
            lines: Vec::new(),
            word_count: 0,
            boxes: Vec::new(),
            paragraphs: Vec::new(),
        });
    }

    // 1. Add background-aware edge padding (16px) to protect symbols at crop boundaries (like '-' or '=')
    let pad = 16.0f64;
    let canvas_w = (orig_w + 2.0 * pad).round() as u32;
    let canvas_h = (orig_h + 2.0 * pad).round() as u32;

    // Sample background color from crop corners
    let bg_pixel = {
        let p1 = image.get_pixel(0, 0);
        let p2 = image.get_pixel(image.width() - 1, 0);
        let p3 = image.get_pixel(0, image.height() - 1);
        let p4 = image.get_pixel(image.width() - 1, image.height() - 1);
        let r = ((p1[0] as u32 + p2[0] as u32 + p3[0] as u32 + p4[0] as u32) / 4) as u8;
        let g = ((p1[1] as u32 + p2[1] as u32 + p3[1] as u32 + p4[1] as u32) / 4) as u8;
        let b = ((p1[2] as u32 + p2[2] as u32 + p3[2] as u32 + p4[2] as u32) / 4) as u8;
        image::Rgba([r, g, b, 255])
    };

    let mut padded_canvas = RgbaImage::from_pixel(canvas_w, canvas_h, bg_pixel);
    image::imageops::overlay(&mut padded_canvas, image, pad as i64, pad as i64);

    // 2. Optimal lightweight scaling: enlarges small crops (<450px) by up to 1.75x so thin strokes gain thickness,
    // while keeping max payload dimension under 1000px so uploads are instant (<300ms) and never time out.
    let max_canvas_dim = canvas_w.max(canvas_h) as f64;
    let target_scale = if max_canvas_dim < 450.0 {
        (750.0 / max_canvas_dim).min(1.75)
    } else if max_canvas_dim > 1000.0 {
        1000.0 / max_canvas_dim
    } else {
        1.0f64
    };

    let upload_image = if (target_scale - 1.0).abs() > 0.05 {
        let tw = (canvas_w as f64 * target_scale).round() as u32;
        let th = (canvas_h as f64 * target_scale).round() as u32;
        image::imageops::resize(
            &padded_canvas,
            tw.max(1),
            th.max(1),
            image::imageops::FilterType::Lanczos3,
        )
    } else {
        padded_canvas
    };

    // Encode image to PNG bytes
    let mut bytes: Vec<u8> = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut bytes);
    upload_image
        .write_to(&mut cursor, image::ImageFormat::Png)
        .map_err(|e| format!("Failed to encode image for Google Lens: {e}"))?;

    // Call Google Lens OCR client with a bounded timeout (15 seconds)
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let res = tauri::async_runtime::block_on(async move {
            let client = chrome_lens_ocr::LensClient::new(None);
            client.process_image_bytes(&bytes, None).await
        });
        let _ = tx.send(res);
    });

    let lens_result = rx
        .recv_timeout(Duration::from_secs(15))
        .map_err(|_| "Google Lens request timed out".to_string())?
        .map_err(|e| format!("Google Lens OCR failed: {e}"))?;

    let mut raw_lines = Vec::new();

    for paragraph in &lens_result.paragraphs {
        for line in &paragraph.lines {
            let mut line_words = Vec::new();

            for word in &line.words {
                let trimmed = word.text.trim();
                if trimmed.is_empty() {
                    continue;
                }

                let (wx, wy, ww, wh) = if let Some(geo) = &word.geometry {
                    let w = (geo.width as f64) * (canvas_w as f64);
                    let h = (geo.height as f64) * (canvas_h as f64);
                    let cx = (geo.center_x as f64) * (canvas_w as f64) - pad;
                    let cy = (geo.center_y as f64) * (canvas_h as f64) - pad;
                    let x = cx - w / 2.0;
                    let y = cy - h / 2.0;
                    (
                        x.max(0.0).min(orig_w),
                        y.max(0.0).min(orig_h),
                        w.max(1.0).min(orig_w),
                        h.max(1.0).min(orig_h),
                    )
                } else {
                    (0.0, 0.0, 0.0, 0.0)
                };

                line_words.push(LensInternalWord {
                    text: trimmed.to_string(),
                    separator: word.separator.clone(),
                    x: wx,
                    y: wy,
                    width: ww,
                    height: wh,
                });
            }

            if line_words.is_empty() {
                continue;
            }

            line_words.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal));

            let (lx, ly, lw, lh) = if let Some(geo) = &line.geometry {
                let w = (geo.width as f64) * (canvas_w as f64);
                let h = (geo.height as f64) * (canvas_h as f64);
                let cx = (geo.center_x as f64) * (canvas_w as f64) - pad;
                let cy = (geo.center_y as f64) * (canvas_h as f64) - pad;
                let x = cx - w / 2.0;
                let y = cy - h / 2.0;
                (
                    x.max(0.0).min(orig_w),
                    y.max(0.0).min(orig_h),
                    w.max(1.0).min(orig_w),
                    h.max(1.0).min(orig_h),
                )
            } else {
                let min_x = line_words.iter().map(|w| w.x).fold(f64::INFINITY, f64::min);
                let max_x = line_words.iter().map(|w| w.x + w.width).fold(f64::NEG_INFINITY, f64::max);
                let min_y = line_words.iter().map(|w| w.y).fold(f64::INFINITY, f64::min);
                let max_y = line_words.iter().map(|w| w.y + w.height).fold(f64::NEG_INFINITY, f64::max);
                (min_x, min_y, (max_x - min_x).max(1.0), (max_y - min_y).max(1.0))
            };

            raw_lines.push(LensInternalLine {
                x: lx,
                y: ly,
                width: lw,
                height: lh,
                words: line_words,
            });
        }
    }

    if raw_lines.is_empty() {
        return Ok(OcrResult {
            full_text: String::new(),
            lines: Vec::new(),
            word_count: 0,
            boxes: Vec::new(),
            paragraphs: Vec::new(),
        });
    }

    // 1. Merge horizontally-aligned line fragments on the same row (e.g. line numbers & code lines)
    let mut lines = raw_lines;
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
                let center_diff = ((lines[i].y + lines[i].height * 0.5) - (lines[j].y + lines[j].height * 0.5)).abs();

                if (v_overlap / min_h) >= 0.50 && center_diff <= (max_h * 0.35).max(min_h * 0.35) {
                    let (left_idx, right_idx) = if lines[i].x <= lines[j].x {
                        (i, j)
                    } else {
                        (j, i)
                    };
                    let left_end = lines[left_idx].x + lines[left_idx].width;
                    let gap = lines[right_idx].x - left_end;

                    // Merge if they are side-by-side on the same visual line (up to column/gutter distance)
                    if gap >= -6.0 && gap <= 180.0 {
                        let mut right_words = lines.remove(right_idx).words;
                        if left_idx == i {
                            lines[i].words.append(&mut right_words);
                        } else {
                            right_words.append(&mut lines[i].words);
                            lines[i].words = right_words;
                        }

                        lines[i].words.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal));
                        let min_x = lines[i].words.iter().map(|w| w.x).fold(f64::INFINITY, f64::min);
                        let max_x = lines[i].words.iter().map(|w| w.x + w.width).fold(f64::NEG_INFINITY, f64::max);
                        let min_y = lines[i].words.iter().map(|w| w.y).fold(f64::INFINITY, f64::min);
                        let max_y = lines[i].words.iter().map(|w| w.y + w.height).fold(f64::NEG_INFINITY, f64::max);
                        lines[i].x = min_x;
                        lines[i].y = min_y;
                        lines[i].width = (max_x - min_x).max(1.0);
                        lines[i].height = (max_y - min_y).max(1.0);

                        merged_any = true;
                        break;
                    }
                }
                j += 1;
            }
            if merged_any {
                break;
            }
            i += 1;
        }
        if !merged_any {
            break;
        }
    }

    // 2. Sort lines top-to-bottom
    lines.sort_by(|a, b| {
        let diff = a.y - b.y;
        if diff.abs() > 4.0 {
            a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal)
        } else {
            a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal)
        }
    });

    // 3. Prevent bounding box collisions between adjacent words in visual highlights
    for line in &mut lines {
        line.words.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal));
        for idx in 0..line.words.len().saturating_sub(1) {
            let right = line.words[idx].x + line.words[idx].width;
            if right > line.words[idx + 1].x {
                let max_w = (line.words[idx + 1].x - line.words[idx].x).max(1.0);
                line.words[idx].width = line.words[idx].width.min(max_w);
            }
        }
    }

    // 4. Estimate character width and base code margin
    let mut total_w = 0.0;
    let mut total_chars = 0;
    for line in &lines {
        for word in &line.words {
            let count = word.text.chars().count();
            if count >= 2 && word.width > 0.0 {
                total_w += word.width;
                total_chars += count;
            }
        }
    }
    let char_w = if total_chars > 0 {
        (total_w / total_chars as f64).max(3.0)
    } else {
        8.0
    };

    let get_code_start_idx = |l: &LensInternalLine| -> usize {
        if let Some(w0) = l.words.first() {
            if w0.text.chars().all(|c| c.is_ascii_digit()) && w0.x < 75.0 && l.words.len() >= 2 {
                let w1 = &l.words[1];
                if (w1.x - (w0.x + w0.width)) >= 10.0 {
                    return 1;
                }
            }
        }
        0
    };

    let base_left_x = lines
        .iter()
        .filter_map(|l| {
            let idx = get_code_start_idx(l);
            l.words.get(idx).map(|w| w.x)
        })
        .fold(f64::INFINITY, f64::min);
    let base_left_x = if base_left_x.is_finite() { base_left_x } else { 0.0 };

    // 4b. Recover dropped thin symbols ('=', '-') by inspecting pixel ink inside gaps
    for line in &mut lines {
        // A. Leading bullet check (e.g. dropped '-' at the start of a list item)
        if let Some(w0) = line.words.first() {
            let is_word_item = w0.text.chars().next().map(|c| c.is_alphabetic()).unwrap_or(false);
            if w0.x >= 14.0 && w0.x <= 50.0 && is_word_item && w0.text != "-" && w0.text != "•" {
                let gap_x0 = (w0.x - 24.0).max(4.0);
                let gap_x1 = w0.x - 2.0;
                if let Some(sym) = inspect_gap_for_symbol(image, gap_x0, gap_x1, line.y, line.height) {
                    if sym == "-" {
                        line.words.insert(
                            0,
                            LensInternalWord {
                                text: "-".to_string(),
                                separator: " ".to_string(),
                                x: gap_x0,
                                y: line.y,
                                width: (gap_x1 - gap_x0).max(4.0),
                                height: line.height,
                            },
                        );
                    }
                }
            }
        }

        // B. Intra-line gaps (e.g. dropped '=' between variable and expression, or '-' in gaps)
        let mut idx = 0;
        while idx + 1 < line.words.len() {
            let w_curr_right = line.words[idx].x + line.words[idx].width;
            let w_next_left = line.words[idx + 1].x;
            let gap = w_next_left - w_curr_right;

            if gap >= (char_w * 1.2).max(10.0) && gap <= 100.0 {
                let prev_text = &line.words[idx].text;
                let next_text = &line.words[idx + 1].text;
                let is_assign_lhs = prev_text.chars().all(|c| c.is_alphanumeric() || c == '_' || c == ',');
                let is_assign_rhs = next_text.starts_with('"')
                    || next_text.starts_with('\'')
                    || next_text.starts_with('[')
                    || next_text.starts_with('{')
                    || next_text.starts_with('(')
                    || next_text.starts_with("raw_input")
                    || next_text.starts_with("input")
                    || next_text.chars().all(|c| c.is_ascii_digit());

                if let Some(sym) = inspect_gap_for_symbol(image, w_curr_right, w_next_left, line.y, line.height) {
                    let recovered_sym = if is_assign_lhs && is_assign_rhs && sym == "-" {
                        "="
                    } else {
                        sym
                    };
                    let new_word = LensInternalWord {
                        text: recovered_sym.to_string(),
                        separator: " ".to_string(),
                        x: w_curr_right + 1.0,
                        y: line.y,
                        width: (gap - 2.0).max(4.0),
                        height: line.height,
                    };
                    line.words.insert(idx + 1, new_word);
                    idx += 1;
                }
            }
            idx += 1;
        }
    }

    // 5. Build final OcrLine records preserving native Google Lens spacing and separators
    let mut ocr_lines = Vec::new();
    for line in lines {
        let code_start = get_code_start_idx(&line);
        let has_gutter = code_start > 0;

        let mut line_str = String::new();
        if has_gutter {
            line_str.push_str(&line.words[0].text);
            line_str.push_str("    ");
        }

        // Prepend code indentation if applicable
        if let Some(first_code_w) = line.words.get(code_start) {
            let indent_px = (first_code_w.x - base_left_x).max(0.0);
            if indent_px >= char_w * 0.7 {
                let spaces = (indent_px / char_w).round() as usize;
                line_str.push_str(&" ".repeat(spaces));
            }
        }

        let mut ocr_words = Vec::new();
        for (i, w) in line.words.iter().enumerate() {
            ocr_words.push(OcrWord {
                text: w.text.clone(),
                x: w.x,
                y: w.y,
                width: w.width,
                height: w.height,
                confidence: 98.0,
            });

            if i >= code_start {
                line_str.push_str(&w.text);
                if i + 1 < line.words.len() {
                    let next_w = &line.words[i + 1];
                    let gap = next_w.x - (w.x + w.width);
                    if gap >= char_w * 1.8 {
                        let spaces = ((gap / char_w).round() as usize).max(2).min(8);
                        line_str.push_str(&" ".repeat(spaces));
                    } else if !w.separator.is_empty() {
                        line_str.push_str(&w.separator);
                    }
                }
            }
        }

        ocr_lines.push(OcrLine {
            text: line_str,
            x: line.x,
            y: line.y,
            width: line.width,
            height: line.height,
            words: ocr_words,
        });
    }

    let full_text = ocr_lines
        .iter()
        .map(|l| l.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    let boxes = ocr_lines
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

    let paragraphs = group_lines_into_paragraphs(&ocr_lines);
    let word_count = ocr_lines.iter().map(|l| l.words.len()).sum();

    Ok(OcrResult {
        full_text,
        lines: ocr_lines,
        word_count,
        boxes,
        paragraphs,
    })
}

/// Inspects an image gap between words or before the first word to recover thin symbols ('=', '-')
/// that may have been missed by neural OCR due to extreme thinness or low color contrast.
fn inspect_gap_for_symbol(
    image: &RgbaImage,
    x0: f64,
    x1: f64,
    y0: f64,
    h: f64,
) -> Option<&'static str> {
    let (img_w, img_h) = (image.width(), image.height());
    let xs = (x0.max(0.0).round() as u32).min(img_w);
    let xe = (x1.max(0.0).round() as u32).min(img_w);
    let gap_w = xe.saturating_sub(xs);
    if gap_w < 5 || gap_w > 120 {
        return None;
    }

    let ys = (y0 + h * 0.20).max(0.0).round() as u32;
    let ye = (y0 + h * 0.80).min(img_h as f64).round() as u32;
    if ye <= ys + 2 {
        return None;
    }

    // Estimate local background color from surrounding boundary
    let sample_x = xs.saturating_sub(2);
    let bg = image.get_pixel(sample_x, (ys + ye) / 2);
    let (bg_r, bg_g, bg_b) = (bg[0] as f32, bg[1] as f32, bg[2] as f32);

    let mut row_ink_counts = Vec::new();
    let mut total_ink = 0;
    for y in ys..ye {
        let mut row_ink = 0;
        for x in xs..xe {
            let p = image.get_pixel(x, y);
            let diff = (p[0] as f32 - bg_r).abs()
                .max((p[1] as f32 - bg_g).abs())
                .max((p[2] as f32 - bg_b).abs());
            if diff > 22.0 {
                row_ink += 1;
                total_ink += 1;
            }
        }
        row_ink_counts.push(row_ink);
    }

    if total_ink < 4 {
        return None;
    }

    let min_row_ink = (gap_w / 3).max(3);
    let active_rows: Vec<usize> = row_ink_counts
        .iter()
        .enumerate()
        .filter(|&(_, &c)| c >= min_row_ink)
        .map(|(i, _)| i)
        .collect();

    if active_rows.is_empty() {
        return None;
    }

    // Check if there are two distinct horizontal bands (equals sign '=')
    // Band 1 in upper half, Band 2 in lower half, with at least 1 row gap between them
    let upper_rows: Vec<usize> = active_rows.iter().copied().filter(|&r| r < row_ink_counts.len() / 2).collect();
    let lower_rows: Vec<usize> = active_rows.iter().copied().filter(|&r| r >= row_ink_counts.len() / 2).collect();

    if !upper_rows.is_empty() && !lower_rows.is_empty() {
        let max_upper = *upper_rows.iter().max().unwrap();
        let min_lower = *lower_rows.iter().min().unwrap();
        if min_lower >= max_upper + 2 {
            return Some("=");
        }
    }

    // Check if there is one continuous horizontal band in the vertical middle (dash '-')
    let mid_y = row_ink_counts.len() / 2;
    let has_mid_ink = active_rows.iter().any(|&r| (r as isize - mid_y as isize).abs() <= 2);
    let top_clean = row_ink_counts.first().copied().unwrap_or(0) == 0;
    let bot_clean = row_ink_counts.last().copied().unwrap_or(0) == 0;

    if has_mid_ink && (top_clean || bot_clean) {
        return Some("-");
    }

    None
}
