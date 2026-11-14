//! Integration with Google Lens Web OCR endpoint (Chromium protocol).
use std::time::Duration;
use image::RgbaImage;
use crate::ocr::{
    group_lines_into_paragraphs, BoundingBox, OcrLine, OcrResult, OcrWord,
};

#[derive(Clone)]
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

fn get_lens_client() -> &'static chrome_lens_ocr::LensClient {
    static CLIENT: std::sync::OnceLock<chrome_lens_ocr::LensClient> = std::sync::OnceLock::new();
    CLIENT.get_or_init(|| chrome_lens_ocr::LensClient::new(None))
}

/// Encodes a padded canvas with smart upscaling (up to 1480px) into fast lossless PNG bytes.
fn encode_canvas_for_lens(padded_canvas: &RgbaImage) -> Result<Vec<u8>, String> {
    let (canvas_w, canvas_h) = (padded_canvas.width(), padded_canvas.height());
    let max_canvas_dim = canvas_w.max(canvas_h) as f64;
    let target_scale = if max_canvas_dim < 500.0 {
        (900.0 / max_canvas_dim).min(2.0)
    } else if max_canvas_dim < 1150.0 {
        (1420.0 / max_canvas_dim).min(1.65)
    } else if max_canvas_dim > 1480.0 {
        1480.0 / max_canvas_dim
    } else {
        1.0f64
    };

    let resized;
    let upload_image: &RgbaImage = if (target_scale - 1.0).abs() > 0.05 {
        let tw = (canvas_w as f64 * target_scale).round() as u32;
        let th = (canvas_h as f64 * target_scale).round() as u32;
        resized = image::imageops::resize(
            padded_canvas,
            tw.max(1),
            th.max(1),
            image::imageops::FilterType::Lanczos3,
        );
        &resized
    } else {
        padded_canvas
    };

    let mut bytes: Vec<u8> = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut bytes);
    upload_image
        .write_to(&mut cursor, image::ImageFormat::Png)
        .map_err(|e| format!("Failed to encode PNG for Google Lens: {e}"))?;
    Ok(bytes)
}

/// Converts a raw `LensResult` into normalized, row-merged, sorted `LensInternalLine`s.
fn parse_lens_lines(
    lens_result: &chrome_lens_ocr::LensResult,
    canvas_w: f64,
    canvas_h: f64,
    pad: f64,
    orig_w: f64,
    orig_h: f64,
) -> Vec<LensInternalLine> {
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
                    let w = (geo.width as f64) * canvas_w;
                    let h = (geo.height as f64) * canvas_h;
                    let cx = (geo.center_x as f64) * canvas_w - pad;
                    let cy = (geo.center_y as f64) * canvas_h - pad;
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
                let w = (geo.width as f64) * canvas_w;
                let h = (geo.height as f64) * canvas_h;
                let cx = (geo.center_x as f64) * canvas_w - pad;
                let cy = (geo.center_y as f64) * canvas_h - pad;
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
                    let (left_idx, right_idx) = if lines[i].x <= lines[j].x { (i, j) } else { (j, i) };
                    let left_end = lines[left_idx].x + lines[left_idx].width;
                    let gap = lines[right_idx].x - left_end;

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

    lines.sort_by(|a, b| {
        let diff = a.y - b.y;
        if diff.abs() > 4.0 {
            a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal)
        } else {
            a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal)
        }
    });

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

    lines
}

/// Fuses OCR results from the filtered pass (`primary`) and unfiltered pass (`secondary`) using
/// geometric bounding-box alignment: fills missing symbols/words in gaps without duplicating
/// overlapping words, and upgrades weaker symbols when the complementary pass read a stronger one.
fn merge_ocr_passes(mut primary: Vec<LensInternalLine>, secondary: Vec<LensInternalLine>) -> Vec<LensInternalLine> {
    let is_strong_op = |t: &str| matches!(t, "=" | "==" | "+" | "->" | "!=" | "<=" | ">=");
    let is_weak_sym = |t: &str| matches!(t, "-" | "–" | "~" | "." | "," | ":");

    for s_line in secondary {
        let match_idx = primary.iter().position(|p_line| {
            let min_h = p_line.height.min(s_line.height);
            if min_h <= 0.0 {
                return false;
            }
            let top = p_line.y.max(s_line.y);
            let bottom = (p_line.y + p_line.height).min(s_line.y + s_line.height);
            let v_overlap = (bottom - top).max(0.0);
            let center_diff = ((p_line.y + p_line.height * 0.5) - (s_line.y + s_line.height * 0.5)).abs();
            (v_overlap / min_h) >= 0.45 || center_diff <= min_h * 0.45
        }).or_else(|| {
            // Fallback: catch slightly shifted duplicate lines that share content on the same visual row
            primary.iter().position(|p_line| {
                let max_h = p_line.height.max(s_line.height);
                let center_diff = ((p_line.y + p_line.height * 0.5) - (s_line.y + s_line.height * 0.5)).abs();
                if center_diff > max_h * 1.15 {
                    return false;
                }
                let h_ov = (p_line.x + p_line.width).min(s_line.x + s_line.width) - p_line.x.max(s_line.x);
                if h_ov <= 0.0 {
                    return false;
                }
                s_line.words.iter().any(|sw| {
                    sw.text.len() >= 3
                        && p_line.words.iter().any(|pw| pw.text.eq_ignore_ascii_case(&sw.text))
                })
            })
        });

        let Some(p_idx) = match_idx else {
            primary.push(s_line);
            continue;
        };

        let p_line = &mut primary[p_idx];
        for sw in s_line.words {
            let sw_right = sw.x + sw.width;
            let sw_cx = sw.x + sw.width * 0.5;

            let overlap_idx = p_line.words.iter().position(|pw| {
                let pw_right = pw.x + pw.width;
                let ov = (pw_right.min(sw_right) - pw.x.max(sw.x)).max(0.0);
                let min_w = pw.width.min(sw.width).max(1.0);
                let pw_cx = pw.x + pw.width * 0.5;
                (ov / min_w) >= 0.25 || (sw_cx - pw_cx).abs() <= min_w * 0.40
            });

            if let Some(wi) = overlap_idx {
                let pw = &mut p_line.words[wi];
                if pw.text != sw.text {
                    if is_weak_sym(&pw.text) && is_strong_op(&sw.text) {
                        pw.text = sw.text;
                    } else if pw.text.contains('-') && sw.text.contains('=') && pw.text.replace('-', "=") == sw.text {
                        pw.text = sw.text;
                    } else if pw.text == "\"" && sw.text == "\"\"" {
                        pw.text = sw.text;
                    } else if pw.text.eq_ignore_ascii_case(&sw.text)
                        && matches!(sw.text.as_str(), "for" | "if" | "elif" | "else" | "while" | "in" | "def" | "return" | "print")
                    {
                        pw.text = sw.text;
                    }
                }
            } else {
                // Skip if an identical word already sits right next to it (shifted box between passes)
                let near_duplicate = p_line.words.iter().any(|pw| {
                    if !pw.text.eq_ignore_ascii_case(&sw.text) {
                        return false;
                    }
                    let pw_right = pw.x + pw.width;
                    let gap = if sw.x >= pw_right {
                        sw.x - pw_right
                    } else if pw.x >= sw_right {
                        pw.x - sw_right
                    } else {
                        0.0
                    };
                    gap <= pw.width.max(sw.width) * 1.5
                });
                if near_duplicate {
                    continue;
                }

                // Missing token in primary pass — insert at sorted x position
                let insert_pos = p_line.words.iter().position(|pw| pw.x > sw.x).unwrap_or(p_line.words.len());
                p_line.words.insert(insert_pos, sw);
                if insert_pos > 0 {
                    let prev_right = p_line.words[insert_pos - 1].x + p_line.words[insert_pos - 1].width;
                    let cur_x = p_line.words[insert_pos].x;
                    if cur_x - prev_right >= 4.0 && p_line.words[insert_pos - 1].separator.is_empty() {
                        p_line.words[insert_pos - 1].separator = " ".to_string();
                    }
                }
            }
        }

        let min_x = p_line.words.iter().map(|w| w.x).fold(f64::INFINITY, f64::min);
        let max_x = p_line.words.iter().map(|w| w.x + w.width).fold(f64::NEG_INFINITY, f64::max);
        p_line.x = min_x;
        p_line.width = (max_x - min_x).max(1.0);
    }

    primary.sort_by(|a, b| {
        let diff = a.y - b.y;
        if diff.abs() > 4.0 {
            a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal)
        } else {
            a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal)
        }
    });

    primary
}

/// Extracts text from an image crop using Google Lens Web API.
/// Runs parallel OCR passes (color-enhanced/B&W + original) and fuses their geometric word boxes.
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

    // 1. Run filtered pass and original pass completely in parallel (filter + encode + Google Lens HTTP)
    let pad = 16.0f64;
    let canvas_w = (orig_w + 2.0 * pad).round() as u32;
    let canvas_h = (orig_h + 2.0 * pad).round() as u32;
    let bg_pixel = estimate_border_bg(image);
    let img_arc = std::sync::Arc::new(image.clone());

    let (tx_enh, rx_enh) = std::sync::mpsc::channel();
    let img_enh = std::sync::Arc::clone(&img_arc);
    std::thread::spawn(move || {
        let res: Result<chrome_lens_ocr::LensResult, String> = (|| {
            let enhanced_crop = boost_chromatic_contrast(&img_enh, bg_pixel);
            let enh_pad_pixel = *enhanced_crop.get_pixel(0, 0);
            let mut enh_canvas = RgbaImage::from_pixel(canvas_w, canvas_h, enh_pad_pixel);
            image::imageops::overlay(&mut enh_canvas, &enhanced_crop, pad as i64, pad as i64);
            let enh_bytes = encode_canvas_for_lens(&enh_canvas)?;
            tauri::async_runtime::block_on(async move {
                get_lens_client()
                    .process_image_bytes(&enh_bytes, None)
                    .await
                    .map_err(|e| format!("Google Lens OCR failed: {e}"))
            })
        })();
        let _ = tx_enh.send(res);
    });

    let (tx_orig, rx_orig) = std::sync::mpsc::channel();
    let img_orig = std::sync::Arc::clone(&img_arc);
    std::thread::spawn(move || {
        let res: Result<chrome_lens_ocr::LensResult, String> = (|| {
            let mut orig_canvas = RgbaImage::from_pixel(canvas_w, canvas_h, bg_pixel);
            image::imageops::overlay(&mut orig_canvas, img_orig.as_ref(), pad as i64, pad as i64);
            let orig_bytes = encode_canvas_for_lens(&orig_canvas)?;
            tauri::async_runtime::block_on(async move {
                get_lens_client()
                    .process_image_bytes(&orig_bytes, None)
                    .await
                    .map_err(|e| format!("Google Lens OCR failed: {e}"))
            })
        })();
        let _ = tx_orig.send(res);
    });

    let enh_res = rx_enh.recv_timeout(Duration::from_secs(15));
    let orig_res = rx_orig.recv_timeout(Duration::from_secs(15));

    let (cw_f, ch_f) = (canvas_w as f64, canvas_h as f64);
    let mut lines = match (enh_res, orig_res) {
        (Ok(Ok(e)), Ok(Ok(o))) => {
            let enh_lines = parse_lens_lines(&e, cw_f, ch_f, pad, orig_w, orig_h);
            let orig_lines = parse_lens_lines(&o, cw_f, ch_f, pad, orig_w, orig_h);
            merge_ocr_passes(enh_lines, orig_lines)
        }
        (Ok(Ok(e)), _) => parse_lens_lines(&e, cw_f, ch_f, pad, orig_w, orig_h),
        (_, Ok(Ok(o))) => parse_lens_lines(&o, cw_f, ch_f, pad, orig_w, orig_h),
        (Ok(Err(e)), _) => return Err(e),
        (Err(_), _) => return Err("Google Lens request timed out".to_string()),
    };

    if lines.is_empty() {
        return Ok(OcrResult {
            full_text: String::new(),
            lines: Vec::new(),
            word_count: 0,
            boxes: Vec::new(),
            paragraphs: Vec::new(),
        });
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

    // 4b. Recover dropped or misread thin symbols ('=', '==', '+', '-', '->', '""') by inspecting pixel ink
    let is_code_context = mode == "code"
        || lines.iter().any(|l| {
            l.words.iter().any(|w| {
                w.text.contains('(')
                    || w.text.contains('[')
                    || w.text.contains('_')
                    || w.text == "for"
                    || w.text == "if"
                    || w.text == "def"
            })
        });

    for line in &mut lines {
        let code_start = get_code_start_idx(line);

        // A. Leading bullet check (e.g. dropped '-' at the start of a list item, including numeric starts like "3D")
        if !is_code_context {
            if let Some(w0) = line.words.first() {
                if w0.x >= 4.0 && w0.x <= 85.0 && !w0.text.starts_with('-') && w0.text != "•" && !w0.text.starts_with('#') {
                    let gap_x0 = (w0.x - 26.0).max(3.0);
                    let gap_x1 = w0.x - 1.5;
                    if let Some((sym, sx, sw)) =
                        inspect_region_for_symbol(image, bg_pixel, gap_x0, gap_x1, line.y, line.height, char_w, false, false)
                    {
                        if sym == "-" {
                            line.words.insert(
                                0,
                                LensInternalWord {
                                    text: "-".to_string(),
                                    separator: " ".to_string(),
                                    x: sx,
                                    y: line.y,
                                    width: sw.max(4.0),
                                    height: line.height,
                                },
                            );
                        }
                    }
                }
            }
        }

        // B. Re-verify standalone '-' tokens that Google Lens may have misread from '=' or '=='
        let is_cond_line = line
            .words
            .get(code_start)
            .map(|w| {
                let t = w.text.to_ascii_lowercase();
                t == "if" || t == "elif" || t == "while"
            })
            .unwrap_or(false);

        let line_has_equals = line.words.iter().any(|w| w.text.contains('='));

        for idx in 0..line.words.len() {
            if is_cond_line && is_code_context && line.words[idx].text.contains('-') {
                // Convert `x-abc` or `x - abc` inside `if`/`elif`/`while` header into `==`
                if line.words[idx].text == "-" && idx >= code_start + 1 {
                    line.words[idx].text = "==".to_string();
                    continue;
                } else if line.words[idx].text.len() >= 3 && !line.words[idx].text.starts_with('-') && !line.words[idx].text.ends_with('-') {
                    line.words[idx].text = line.words[idx].text.replacen('-', "==", 1);
                    continue;
                }
            }
            if line.words[idx].text == "-" && is_code_context {
                let wx0 = (line.words[idx].x - 1.0).max(0.0);
                let wx1 = (line.words[idx].x + line.words[idx].width + 1.0).min(orig_w);
                let detected = inspect_region_for_symbol(
                    image,
                    bg_pixel,
                    wx0,
                    wx1,
                    line.y - 2.0,
                    line.height + 4.0,
                    char_w,
                    true,
                    true,
                );
                if is_cond_line && idx >= code_start + 1 && line.words[idx].width >= char_w * 1.15 {
                    line.words[idx].text = "==".to_string();
                } else if idx == code_start + 1 && !line_has_equals {
                    let prev_ok = line.words[idx - 1]
                        .text
                        .chars()
                        .all(|c| c.is_alphanumeric() || c == '_');
                    let next_ok = line
                        .words
                        .get(idx + 1)
                        .map(|w| {
                            w.text
                                .chars()
                                .next()
                                .map(|c| c.is_alphanumeric() || c == '_' || c == '[' || c == '"' || c == '\'')
                                .unwrap_or(false)
                        })
                        .unwrap_or(false);
                    if prev_ok && next_ok {
                        line.words[idx].text = "=".to_string();
                    }
                } else if let Some((sym, _, _)) = detected {
                    if sym == "=" || sym == "==" {
                        line.words[idx].text = if is_cond_line { "==".to_string() } else { "=".to_string() };
                    }
                }
            }
        }

        // C. Intra-line gaps (recover dropped '=', '==', '+', '-', '->' in genuine gaps)
        let mut idx = 0;
        while idx + 1 < line.words.len() {
            let w_curr_right = line.words[idx].x + line.words[idx].width;
            let w_next_left = line.words[idx + 1].x;
            let gap = w_next_left - w_curr_right;

            if gap >= 6.0 && gap <= 110.0 {
                let prev_text = line.words[idx].text.clone();
                let next_text = line.words[idx + 1].text.clone();
                let prev_lower = prev_text.to_ascii_lowercase();
                let prev_already_op = prev_text.ends_with('=')
                    || prev_text.ends_with('-')
                    || prev_text.ends_with('+')
                    || prev_text.ends_with('(')
                    || prev_text.ends_with('[')
                    || prev_text.ends_with(':')
                    || prev_text.ends_with(',')
                    || prev_text.starts_with('#')
                    || prev_text.starts_with('"')
                    || prev_text.starts_with('\'')
                    || prev_lower == "in"
                    || prev_lower == "for"
                    || prev_lower == "if"
                    || prev_lower == "elif"
                    || prev_lower == "while"
                    || prev_lower == "def"
                    || prev_lower == "return"
                    || prev_lower == "print";
                let next_already_op = next_text.starts_with('=')
                    || next_text.starts_with('-')
                    || next_text.starts_with('+')
                    || next_text.starts_with('>')
                    || next_text.starts_with(')')
                    || next_text.starts_with(']')
                    || next_text.starts_with(':')
                    || next_text.starts_with(',')
                    || next_text.starts_with('.')
                    || next_text.starts_with('#');

                // Do not recover code operators inside string literals (count quotes before idx)
                let quotes_before: usize = line.words[..=idx]
                    .iter()
                    .map(|w| w.text.chars().filter(|&c| c == '"' || c == '\'').count())
                    .sum();
                let inside_quotes = is_code_context && (quotes_before % 2 == 1);

                if !prev_already_op && !next_already_op && !inside_quotes {
                    let is_assign_lhs = idx <= code_start + 2
                        && !line.words.iter().any(|w| w.text == "=" || w.text == "==")
                        && line.words[code_start..=idx].iter().all(|w| {
                            w.text != "print"
                                && w.text != "for"
                                && w.text != "if"
                                && w.text
                                    .chars()
                                    .all(|c| c.is_alphanumeric() || c == '_' || c == ',')
                        });
                    let is_assign_rhs = next_text.starts_with('"')
                        || next_text.starts_with('\'')
                        || next_text.starts_with('[')
                        || next_text.starts_with('{')
                        || next_text.starts_with('(')
                        || next_text
                            .chars()
                            .next()
                            .map(|c| c.is_alphanumeric() || c == '_')
                            .unwrap_or(false);

                    let inset = if gap >= 14.0 { 2.0 } else { 1.0 };
                    if let Some((sym, sx, sw)) = inspect_region_for_symbol(
                        image,
                        bg_pixel,
                        w_curr_right + inset,
                        w_next_left - inset,
                        line.y - 2.0,
                        line.height + 4.0,
                        char_w,
                        is_code_context,
                        false,
                    ) {
                        let allow = if is_code_context {
                            if sym == "=" || sym == "==" || sym == "-" {
                                (is_assign_lhs && is_assign_rhs) || is_cond_line
                            } else {
                                sym == "+"
                            }
                        } else {
                            (sym == "->" || sym == "-") && gap >= 15.0
                        };

                        if allow {
                            let recovered_sym = if is_code_context && is_cond_line {
                                if sw >= char_w * 1.20 { "==" } else { "=" }
                            } else if is_code_context && (sym == "-" || sym == "==") && is_assign_lhs {
                                "="
                            } else {
                                sym
                            };

                            let left_pad = sx - w_curr_right;
                            let right_pad = w_next_left - (sx + sw);
                            if left_pad < char_w * 0.40 {
                                line.words[idx].separator = String::new();
                            } else if line.words[idx].separator.is_empty() {
                                line.words[idx].separator = " ".to_string();
                            }
                            let right_sep = if right_pad >= char_w * 0.40 {
                                " ".to_string()
                            } else {
                                String::new()
                            };

                            let new_word = LensInternalWord {
                                text: recovered_sym.to_string(),
                                separator: right_sep,
                                x: sx,
                                y: line.y,
                                width: sw.max(4.0),
                                height: line.height,
                            };
                            line.words.insert(idx + 1, new_word);
                            idx += 1;
                        }
                    }
                }
            }
            idx += 1;
        }

        // D. Trailing symbols after the last word on a line (e.g. 'encoding= ""' or 'posicion= 0' / 'suma = 0')
        if is_code_context {
            // Fix Google Lens misreading `""` after `=` as `"` + `0` (e.g. `espacios="0` -> `espacios=""`)
            let w_len = line.words.len();
            if w_len >= 3
                && line.words[w_len - 1].text == "0"
                && line.words[w_len - 2].text == "\""
                && line.words[w_len - 3].text.ends_with('=')
                && line.words[w_len - 1].x - line.words[w_len - 2].x <= char_w * 2.0
            {
                let qx = line.words[w_len - 2].x;
                let qw = (line.words[w_len - 1].x + line.words[w_len - 1].width) - qx;
                if let Some((sym, _, _)) = inspect_region_for_symbol(
                    image,
                    bg_pixel,
                    qx + 1.0,
                    qx + qw + 2.0,
                    line.y - 2.0,
                    line.height + 4.0,
                    char_w,
                    true,
                    true,
                ) {
                    if sym == "\"\"" {
                        line.words.pop();
                        if let Some(lw) = line.words.last_mut() {
                            lw.text = "\"\"".to_string();
                            lw.width = qw;
                        }
                    }
                }
            }

            if let Some(last_w) = line.words.last().cloned() {
                let last_right = last_w.x + last_w.width;
                let scan_end = (last_right + char_w * 5.0).min(orig_w);
                if last_w.text.ends_with('-') && last_w.text.len() >= 2 && line.words.len() == code_start + 1 {
                    if let Some(lw) = line.words.last_mut() {
                        lw.text.pop();
                        lw.text.push('=');
                    }
                }
                if scan_end > last_right + 4.0 {
                    let updated_last = line.words.last().cloned().unwrap();
                    if updated_last.text.ends_with('=') {
                        if let Some((sym, sx, sw)) = inspect_region_for_symbol(
                            image,
                            bg_pixel,
                            last_right + 1.5,
                            scan_end,
                            line.y - 2.0,
                            line.height + 4.0,
                            char_w,
                            true,
                            true,
                        ) {
                            if sym == "\"\"" {
                                if let Some(lw) = line.words.last_mut() {
                                    if sx - last_right >= char_w * 0.40 {
                                        lw.separator = " ".to_string();
                                    }
                                }
                                line.words.push(LensInternalWord {
                                    text: "\"\"".to_string(),
                                    separator: String::new(),
                                    x: sx,
                                    y: line.y,
                                    width: sw.max(6.0),
                                    height: line.height,
                                });
                            }
                        }
                    } else if line.words.len() == code_start + 1
                        && updated_last.text != "else"
                        && updated_last.text.chars().all(|c| c.is_alphanumeric() || c == '_')
                    {
                        let eq_end = (last_right + char_w * 2.5).min(orig_w);
                        if let Some((sym, sx, sw)) = inspect_region_for_symbol(
                            image,
                            bg_pixel,
                            last_right + 1.0,
                            eq_end,
                            line.y - 2.0,
                            line.height + 4.0,
                            char_w,
                            true,
                            true,
                        ) {
                            if sym == "=" || sym == "-" {
                                if let Some(lw) = line.words.last_mut() {
                                    lw.separator = if sx - last_right >= char_w * 0.40 {
                                        " ".to_string()
                                    } else {
                                        String::new()
                                    };
                                }
                                let rhs_start = sx + sw + 1.5;
                                let rhs_tok = inspect_trailing_literal(
                                    image,
                                    bg_pixel,
                                    rhs_start,
                                    (rhs_start + char_w * 3.2).min(orig_w),
                                    line.y - 2.0,
                                    line.height + 4.0,
                                );
                                let eq_sep = if rhs_tok.as_ref().map(|(_, rx, _)| *rx - (sx + sw) >= char_w * 0.40).unwrap_or(false) {
                                    " ".to_string()
                                } else {
                                    String::new()
                                };
                                line.words.push(LensInternalWord {
                                    text: "=".to_string(),
                                    separator: eq_sep,
                                    x: sx,
                                    y: line.y,
                                    width: sw.max(5.0),
                                    height: line.height,
                                });
                                if let Some((lit, rx, rw)) = rhs_tok {
                                    line.words.push(LensInternalWord {
                                        text: lit.to_string(),
                                        separator: String::new(),
                                        x: rx,
                                        y: line.y,
                                        width: rw.max(5.0),
                                        height: line.height,
                                    });
                                }
                            }
                        }
                    }
                }
            }
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

/// Estimates dominant background color from an interior + border sample grid using median per channel.
fn estimate_border_bg(image: &RgbaImage) -> image::Rgba<u8> {
    let (w, h) = (image.width(), image.height());
    if w == 0 || h == 0 {
        return image::Rgba([0, 0, 0, 255]);
    }
    let mut rs = Vec::new();
    let mut gs = Vec::new();
    let mut bs = Vec::new();
    let step_x = (w / 12).max(1);
    let step_y = (h / 12).max(1);
    for y in (0..h).step_by(step_y as usize) {
        for x in (0..w).step_by(step_x as usize) {
            let p = image.get_pixel(x, y);
            rs.push(p[0]);
            gs.push(p[1]);
            bs.push(p[2]);
        }
    }
    rs.sort_unstable();
    gs.sort_unstable();
    bs.sort_unstable();
    let mid = rs.len() / 2;
    image::Rgba([rs[mid], gs[mid], bs[mid], 255])
}

/// 3-step color-based foreground/background separation and contrast boost:
/// 1. Estimates global background (`bg`) and measures full Euclidean RGB distance `d_chroma = ||P - bg||`
///    across all pixels to identify candidate text strokes without OCR.
/// 2. Determines the dominant foreground chromatic contrast `fg_peak` (90th percentile of stroke pixels):
///    - If `fg_peak < 110` (low-contrast / iso-luminant image where text and background have similar brightness,
///      such as magenta `- Rapid` on salmon background), normalizes the image to 100% Black-on-White (`0..255`)
///      along the `BG -> FG` distance axis.
///    - Otherwise (multi-colored syntax-highlighted code or slides with overlays), preserves background and
///      applies a targeted chromatic luminance lift so low-luminance colored glyphs (pink `=`, purple `0`,
///      pastel bullets/arrows) gain strong luminance contrast without turning highlight boxes into dark blocks.
fn boost_chromatic_contrast(image: &RgbaImage, bg: image::Rgba<u8>) -> RgbaImage {
    let (w, h) = (image.width(), image.height());
    if w == 0 || h == 0 {
        return image.clone();
    }
    let (bg_r, bg_g, bg_b) = (bg[0] as f32, bg[1] as f32, bg[2] as f32);
    let bg_y = 0.299 * bg_r + 0.587 * bg_g + 0.114 * bg_b;
    let is_dark = bg_y < 128.0;

    // Step 1: Measure Euclidean chromatic distance of candidate stroke pixels (subsampled 3x3 for O(N/9) speed)
    let mut stroke_dists = Vec::with_capacity(((w * h) / 36) as usize);
    let step = if w > 120 && h > 120 { 3 } else { 1 };
    for y in (0..h).step_by(step) {
        for x in (0..w).step_by(step) {
            let p = image.get_pixel(x, y);
            let dr = (p[0] as f32) - bg_r;
            let dg = (p[1] as f32) - bg_g;
            let db = (p[2] as f32) - bg_b;
            let d_sq = dr * dr + dg * dg + db * db;
            if d_sq > 324.0 {
                stroke_dists.push(d_sq.sqrt());
            }
        }
    }

    let fg_peak = if stroke_dists.len() >= 8 {
        let nth = (stroke_dists.len() * 90) / 100;
        let (_, val, _) = stroke_dists.select_nth_unstable_by(nth, |a, b| {
            a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal)
        });
        *val
    } else {
        200.0
    };

    // Step 2a: Iso-luminant / low-contrast color pair (e.g. `- Rapid` pink on salmon where fg_peak ~ 65)
    // Convert 100% to pure Black-on-White based on BG -> FG chromatic distance!
    if fg_peak < 105.0 {
        let mut out = RgbaImage::from_pixel(w, h, image::Rgba([255, 255, 255, 255]));
        let noise_floor = 16.0f32;
        let target_fg = (fg_peak * 0.85).max(32.0);
        let denom = (target_fg - noise_floor).max(10.0);
        for (src, dst) in image.as_raw().chunks_exact(4).zip(out.as_mut().chunks_exact_mut(4)) {
            let dr = (src[0] as f32) - bg_r;
            let dg = (src[1] as f32) - bg_g;
            let db = (src[2] as f32) - bg_b;
            let d = (dr * dr + dg * dg + db * db).sqrt();
            let t = ((d - noise_floor) / denom).clamp(0.0, 1.0);
            let v = (255.0 * (1.0 - t)).round() as u8;
            dst[0] = v;
            dst[1] = v;
            dst[2] = v;
            dst[3] = 255;
        }
        return out;
    }

    // Step 2b: Multi-color code / standard contrast — boost low-luminance chromatic glyphs
    let mut out = image.clone();
    for p in out.as_mut().chunks_exact_mut(4) {
        let (r, g, b) = (p[0] as f32, p[1] as f32, p[2] as f32);
        if is_dark {
            let dr = (r - bg_r).max(0.0);
            let dg = (g - bg_g).max(0.0);
            let db = (b - bg_b).max(0.0);
            // Specifically lift saturated red/pink/magenta/purple glyphs where green is low
            let chroma = dr.max(db) - dg;
            if chroma > 20.0 {
                let lift = (chroma - 20.0) * 0.68;
                p[0] = (r + lift * 0.60).min(255.0) as u8;
                p[1] = (g + lift).min(255.0) as u8;
                p[2] = (b + lift * 0.80).min(255.0) as u8;
            }
        } else {
            let dr = (r - bg_r).abs();
            let dg = (g - bg_g).abs();
            let db = (b - bg_b).abs();
            let d_max = dr.max(dg).max(db);
            if d_max > 20.0 {
                let drop = (d_max - 20.0) * 0.55;
                p[0] = (r - drop).max(0.0) as u8;
                p[1] = (g - drop).max(0.0) as u8;
                p[2] = (b - drop).max(0.0) as u8;
            }
        }
    }
    out
}

/// Inspects a horizontal region `[x0, x1]` on a line `[y0, y0 + h]` to classify missed or misread symbols
/// ('=', '==', '+', '-', '->', '""'), returning `(symbol_str, ink_x, ink_width)`.
fn inspect_region_for_symbol(
    image: &RgbaImage,
    global_bg: image::Rgba<u8>,
    x0: f64,
    x1: f64,
    y0: f64,
    h: f64,
    char_w: f64,
    is_code: bool,
    allow_boundary: bool,
) -> Option<(&'static str, f64, f64)> {
    let (img_w, img_h) = (image.width(), image.height());
    let xs = (x0.max(0.0).round() as u32).min(img_w);
    let xe = (x1.max(0.0).round() as u32).min(img_w);
    let ys = (y0.max(0.0).round() as u32).min(img_h);
    let ye = ((y0 + h).max(0.0).round() as u32).min(img_h);

    let w_box = xe.saturating_sub(xs) as usize;
    let h_box = ye.saturating_sub(ys) as usize;
    if w_box < 4 || w_box > 130 || h_box < 6 {
        return None;
    }

    let (bg_r, bg_g, bg_b) = (global_bg[0] as f32, global_bg[1] as f32, global_bg[2] as f32);
    let bg_y = 0.299 * bg_r + 0.587 * bg_g + 0.114 * bg_b;

    let mut row_lum_diff = vec![0.0f32; h_box];
    let mut row_ink = vec![0usize; h_box];
    let mut col_ink = vec![0usize; w_box];

    for r_idx in 0..h_box {
        let y = ys + r_idx as u32;
        for c_idx in 0..w_box {
            let x = xs + c_idx as u32;
            let p = image.get_pixel(x, y);
            let (r, g, b) = (p[0] as f32, p[1] as f32, p[2] as f32);
            let d_max = (r - bg_r).abs().max((g - bg_g).abs()).max((b - bg_b).abs());
            let yv = 0.299 * r + 0.587 * g + 0.114 * b;
            let dy = (yv - bg_y).abs();
            let is_blue_border = if bg_y < 128.0 {
                // Highlight borders are cool blue/cyan (b > r + 4, g > r - 8) or neutral gray-blue (|r - g| < 16 && b >= r - 2)
                // whereas Monokai operators are warm magenta/pink/purple (r > g + 22)
                (b > r + 4.0 && g > r - 8.0) || ((r - g).abs() < 16.0 && b >= r - 2.0)
            } else {
                b > r + 15.0 && (bg_r - r) > 35.0
            };
            if !is_blue_border && d_max > 20.0 {
                row_lum_diff[r_idx] += dy;
                row_ink[r_idx] += 1;
                col_ink[c_idx] += 1;
            }
        }
    }

    // Only strip 1px edge columns if they are tall (> 60% of h_box, i.e. vertical letter edge bleed)
    if !allow_boundary && w_box >= 6 {
        let tall_thresh = (h_box * 60) / 100;
        if col_ink[0] >= tall_thresh {
            col_ink[0] = 0;
        }
        if col_ink[w_box - 1] >= tall_thresh {
            col_ink[w_box - 1] = 0;
        }
    }

    // Suppress isolated 1-2px noise columns (e.g. JPEG artifacts or border corners) so they don't inflate island count
    if w_box >= 5 {
        let raw_cols = col_ink.clone();
        for c in 0..w_box {
            if raw_cols[c] == 1 {
                let left = if c > 0 { raw_cols[c - 1] } else { 0 };
                let right = if c + 1 < w_box { raw_cols[c + 1] } else { 0 };
                if left == 0 && right == 0 {
                    col_ink[c] = 0;
                }
            }
        }
    }

    let active_cols: Vec<usize> = (0..w_box).filter(|&c| col_ink[c] >= 1).collect();
    if active_cols.len() < 3 {
        return None;
    }
    let first_col = *active_cols.first().unwrap();
    let last_col = *active_cols.last().unwrap();
    let ink_w = last_col - first_col + 1;
    let sym_x = xs as f64 + first_col as f64;
    let sym_w = ink_w as f64;

    // Measure longest contiguous horizontal run of active columns
    let mut max_contig_w = 0usize;
    let mut cur_contig = 0usize;
    let mut islands = 0usize;
    for &c in &col_ink {
        if c > 0 {
            if cur_contig == 0 {
                islands += 1;
            }
            cur_contig += 1;
            if cur_contig > max_contig_w {
                max_contig_w = cur_contig;
            }
        } else {
            cur_contig = 0;
        }
    }

    let min_row_c = (max_contig_w / 3).max(2);
    let active_rows: Vec<usize> = (0..h_box).filter(|&r| row_ink[r] >= min_row_c).collect();
    if active_rows.is_empty() {
        return None;
    }
    let first_row = *active_rows.first().unwrap();
    let last_row = *active_rows.last().unwrap();
    let ink_h = last_row - first_row + 1;

    // 1. Quotes '""': ONLY when allow_boundary is true (trailing check after '='), 1 or 2 short vertical islands in upper 62%
    let upper_cut = ((h_box as f64) * 0.62).round() as usize;
    let lower_cut = ((h_box as f64) * 0.65).round() as usize;
    let upper_ink: usize = row_ink.iter().take(upper_cut.min(h_box)).sum();
    let lower_ink: usize = row_ink.iter().skip(lower_cut.min(h_box)).sum();
    if allow_boundary
        && is_code
        && ((islands >= 2 && ink_w >= 5) || (islands == 1 && ink_w >= 10))
        && ink_w <= 18
        && upper_ink >= 4
        && lower_ink == 0
        && last_row <= upper_cut
    {
        return Some(("\"\"", sym_x, sym_w));
    }

    // All horizontal symbols ('=', '==', '+', '-', '->') require a contiguous horizontal stroke of at least 4 columns
    if max_contig_w < 4 || islands > 2 {
        return None;
    }

    let max_col_h = col_ink.iter().copied().max().unwrap_or(0);

    // 2. Bimodal horizontal bars ('=' or '=='): two peaks in row_lum_diff separated by a valley
    if is_code && ink_h <= 8 && max_col_h <= 8 {
        let r_start = ((h_box as f64) * 0.12).round() as usize;
        let r_end = ((h_box as f64) * 0.90).round() as usize;
        let mut peaks: Vec<(usize, f32)> = Vec::new();
        for r in r_start.max(1)..r_end.min(h_box.saturating_sub(1)) {
            let val = row_lum_diff[r];
            if val > 22.0 && val >= row_lum_diff[r - 1] && val >= row_lum_diff[r + 1] {
                if peaks.last().map(|&(pr, _)| r - pr >= 2).unwrap_or(true) {
                    peaks.push((r, val));
                } else if let Some(last) = peaks.last_mut() {
                    if val > last.1 {
                        *last = (r, val);
                    }
                }
            }
        }
        if peaks.len() >= 2 {
            let (p1_r, p1_v) = peaks[0];
            let (p2_r, p2_v) = *peaks.last().unwrap();
            if p2_r >= p1_r + 2 && p2_r <= p1_r + 7 {
                let valley_v = row_lum_diff[(p1_r + 1)..p2_r]
                    .iter()
                    .copied()
                    .fold(f32::INFINITY, f32::min);
                if valley_v < 0.88 * p1_v.min(p2_v) {
                    let tok = if sym_w >= (char_w * 1.45).max(12.5) {
                        "=="
                    } else {
                        "="
                    };
                    return Some((tok, sym_x, sym_w));
                }
            }
        }
        // Fallback when 4:2:0 vertical subsampling merges the 2 bars of '=' into a 4-8px thick magenta block
        if ink_h >= 4 && ink_h <= 8 && max_contig_w >= 6 && ink_w <= 14 && first_row >= 1 && last_row + 1 < h_box {
            return Some(("=", sym_x, sym_w));
        }
    }

    // 3. Plus sign '+': single contiguous component with taller vertical stem in middle columns + horizontal bar
    if is_code
        && islands == 1
        && max_col_h >= 5
        && max_contig_w >= 5
        && ink_w <= 12
        && ink_h >= 5
        && ink_h <= 12
        && (ink_h as isize - ink_w as isize).abs() <= 3
    {
        let mid_c = (first_col + last_col) / 2;
        let c0 = mid_c.saturating_sub(1);
        let c1 = (mid_c + 1).min(w_box - 1);
        let center_h = col_ink[c0..=c1].iter().copied().max().unwrap_or(0);
        let edge_h = col_ink[first_col].max(col_ink[last_col]);
        if center_h >= edge_h + 3 && center_h >= 5 && edge_h <= 3 {
            return Some(("+", sym_x, sym_w));
        }
    }

    // 4. Arrow '->': single contiguous wide stroke with thin stem on left half and taller arrowhead on right half
    if islands == 1 && max_contig_w >= 9 && ink_w >= 9 && ink_w <= 28 {
        let half = active_cols.len() / 2;
        let left_max_h = active_cols[..half].iter().map(|&c| col_ink[c]).max().unwrap_or(0);
        let right_max_h = active_cols[half..].iter().map(|&c| col_ink[c]).max().unwrap_or(0);
        if right_max_h >= left_max_h + 2 && right_max_h >= 4 && right_max_h <= 10 && left_max_h >= 1 && left_max_h <= 4 {
            return Some(("->", sym_x, sym_w));
        }
    }

    // 5. Dash '-': single contiguous horizontal bar in vertical middle
    if islands == 1 && ink_h <= 6 && max_col_h <= 6 && max_contig_w >= 4 && ink_w <= 15 && ink_w >= ink_h {
        let top_clean = row_ink.first().copied().unwrap_or(0) == 0;
        let bot_clean = row_ink.last().copied().unwrap_or(0) == 0;
        if top_clean || bot_clean {
            return Some(("-", sym_x, sym_w));
        }
    }

    None
}

/// Checks if a trailing region after `=` on a short assignment line contains `""` or `0`.
fn inspect_trailing_literal(
    image: &RgbaImage,
    bg: image::Rgba<u8>,
    x0: f64,
    x1: f64,
    y0: f64,
    h: f64,
) -> Option<(&'static str, f64, f64)> {
    if let Some((sym, sx, sw)) = inspect_region_for_symbol(image, bg, x0, x1, y0, h, 8.0, true, true) {
        if sym == "\"\"" {
            return Some(("\"\"", sx, sw));
        }
    }
    let (img_w, img_h) = (image.width(), image.height());
    let xs = (x0.max(0.0).round() as u32).min(img_w);
    let xe = (x1.max(0.0).round() as u32).min(img_w);
    let ys = (y0.max(0.0).round() as u32).min(img_h);
    let ye = ((y0 + h).max(0.0).round() as u32).min(img_h);
    let w_box = xe.saturating_sub(xs) as usize;
    let h_box = ye.saturating_sub(ys) as usize;
    if w_box < 4 || h_box < 6 {
        return None;
    }
    let (bg_r, bg_g, bg_b) = (bg[0] as f32, bg[1] as f32, bg[2] as f32);
    let bg_y = 0.299 * bg_r + 0.587 * bg_g + 0.114 * bg_b;
    let mut col_ink = vec![0usize; w_box];
    let mut row_ink = vec![0usize; h_box];
    for r_idx in 0..h_box {
        for c_idx in 0..w_box {
            let p = image.get_pixel(xs + c_idx as u32, ys + r_idx as u32);
            let (r, g, b) = (p[0] as f32, p[1] as f32, p[2] as f32);
            let d_max = (r - bg_r).abs().max((g - bg_g).abs()).max((b - bg_b).abs());
            let is_blue_border = if bg_y < 128.0 {
                // Only filter cyan/gray-blue highlight borders where green >= red - 2 and blue < red + 35 (sparing purple '0' where blue >= red + 35)
                (b > r + 4.0 && b < r + 35.0 && g >= r - 2.0) || ((r - g).abs() < 12.0 && b >= r && b < r + 28.0)
            } else {
                b > r + 15.0 && (bg_r - r) > 35.0
            };
            if !is_blue_border && d_max > 25.0 {
                col_ink[c_idx] += 1;
                row_ink[r_idx] += 1;
            }
        }
    }
    let active_cols: Vec<usize> = (0..w_box).filter(|&c| col_ink[c] >= 2).collect();
    let active_rows: Vec<usize> = (0..h_box).filter(|&r| row_ink[r] >= 2).collect();
    if active_cols.len() >= 3 && active_rows.len() >= (h_box * 45) / 100 {
        let fc = *active_cols.first().unwrap();
        let lc = *active_cols.last().unwrap();
        let iw = lc - fc + 1;
        if iw >= 4 && iw <= 12 {
            return Some(("0", xs as f64 + fc as f64, iw as f64));
        }
    }
    None
}
