use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct OcrWord {
    pub text: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
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


/// Detects whether an image has a predominantly dark background (e.g. dark mode editor or terminal).
pub fn is_dark_image(image: &image::RgbaImage) -> bool {
    let (w, h) = (image.width(), image.height());
    if w == 0 || h == 0 {
        return false;
    }

    let step = ((w * h) / 10000).max(1) as usize;
    let mut total_luma: u64 = 0;
    let mut count: u64 = 0;

    let raw = image.as_raw();
    let num_pixels = (w * h) as usize;
    for i in (0..num_pixels).step_by(step) {
        let idx = i * 4;
        if idx + 3 < raw.len() {
            let a = raw[idx + 3];
            if a > 32 {
                let r = raw[idx] as u32;
                let g = raw[idx + 1] as u32;
                let b = raw[idx + 2] as u32;
                let luma = (r * 299 + g * 587 + b * 114) / 1000;
                total_luma += luma as u64;
                count += 1;
            }
        }
    }

    if count == 0 {
        return false;
    }
    (total_luma / count) < 128
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

/// Enhances contrast of an image by stretching the dynamic range of pixel luminances.
pub fn enhance_contrast(image: &mut image::RgbaImage) {
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
                for c in 0..3 {
                    let val = (pixel[c].saturating_sub(min_luma) as f32 / range * 255.0).clamp(0.0, 255.0);
                    pixel[c] = val as u8;
                }
            }
        }
    }
}

/// Preprocesses screenshot crop for optimal code OCR precision:
/// - Inverts dark theme images to black-on-white.
/// - Enhances contrast across color spectrum.
/// - Upscales small screen text (font height 8-16px) by 2x using Lanczos3 interpolation.
/// - Restricts dimensions to fit within WinRT OCR max size (2500px).
pub fn preprocess_for_ocr(image: &image::RgbaImage) -> (image::RgbaImage, f64) {
    let (w, h) = (image.width(), image.height());
    if w == 0 || h == 0 {
        return (image.clone(), 1.0);
    }

    let mut prepared = if is_dark_image(image) {
        invert_image(image)
    } else {
        image.clone()
    };

    enhance_contrast(&mut prepared);

    const MAX_OCR_DIM: f64 = 2500.0;
    let scale = if (w as f64 * 2.0) <= MAX_OCR_DIM && (h as f64 * 2.0) <= MAX_OCR_DIM && (w < 1600 || h < 1600) {
        2.0
    } else if (w as f64) > MAX_OCR_DIM || (h as f64) > MAX_OCR_DIM {
        let factor = MAX_OCR_DIM / (w.max(h) as f64);
        (factor * 100.0).floor() / 100.0
    } else {
        1.0
    };

    if (scale - 1.0).abs() > 0.001 {
        let new_w = ((w as f64) * scale).round() as u32;
        let new_h = ((h as f64) * scale).round() as u32;
        let resized = image::imageops::resize(&prepared, new_w, new_h, image::imageops::FilterType::Lanczos3);
        (resized, scale)
    } else {
        (prepared, 1.0)
    }
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

    s
}

/// Coalesces raw OCR lines (resolving line-split artifacts), preserves code indentation,
/// reconstructs token spacing, detects blank lines, and builds structured full text and bounding boxes.
pub fn coalesce_and_structure_lines(mut raw_lines: Vec<OcrLine>) -> (Vec<OcrLine>, String, Vec<BoundingBox>) {
    // 1. Normalize punctuation on all words
    for line in &mut raw_lines {
        for word in &mut line.words {
            word.text = normalize_code_punctuation(&word.text);
        }
        line.words.retain(|w| !w.text.trim().is_empty());
    }
    raw_lines.retain(|l| !l.words.is_empty());

    if raw_lines.is_empty() {
        return (Vec::new(), String::new(), Vec::new());
    }

    // 2. Coalesce lines that are horizontally adjacent fragments of the same visual line
    // (resolving OCR horizontal split artifacts without falsely merging tightly-spaced consecutive lines)
    let mut lines: Vec<OcrLine> = Vec::new();
    for mut r_line in raw_lines {
        let mut merged = false;
        for existing in &mut lines {
            let top = existing.y.max(r_line.y);
            let bottom = (existing.y + existing.height).min(r_line.y + r_line.height);
            let v_overlap = (bottom - top).max(0.0);
            let min_h = existing.height.min(r_line.height);

            let center_1 = existing.y + existing.height * 0.5;
            let center_2 = r_line.y + r_line.height * 0.5;
            let center_diff = (center_1 - center_2).abs();

            // True horizontal fragments of the same visual line:
            // - Overlap vertically by >= 50%
            // - Vertical centers are very close (<= 35% of min height)
            // - No word in r_line horizontally collides with any word in existing
            let is_same_line = min_h > 0.0
                && (v_overlap / min_h) >= 0.50
                && center_diff <= min_h * 0.35
                && !words_collide_horizontally(&existing.words, &r_line.words);

            if is_same_line {
                existing.words.append(&mut r_line.words);
                existing.words.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal));
                let (nx, ny, nw, nh) = compute_words_bounding_box(&existing.words);
                existing.x = nx;
                existing.y = ny;
                existing.width = nw;
                existing.height = nh;
                merged = true;
                break;
            }
        }
        if !merged {
            r_line.words.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal));
            lines.push(r_line);
        }
    }

    // 3. Sort lines top-to-bottom by y coordinate
    lines.sort_by(|a, b| a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal));

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

    // 5. Compute base left margin of the block
    let base_left_x = lines
        .iter()
        .filter_map(|l| l.words.first().map(|w| w.x))
        .fold(f64::INFINITY, f64::min);
    let base_left_x = if base_left_x.is_finite() { base_left_x } else { 0.0 };

    // Collect non-zero indentation offsets across lines
    let mut non_zero_indents: Vec<f64> = lines
        .iter()
        .filter_map(|l| l.words.first().map(|w| w.x - base_left_x))
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

    // 6. Format each line with leading indentation and token spacing
    for line in &mut lines {
        let first_word = &line.words[0];
        let indent_px = (first_word.x - base_left_x).max(0.0);
        let indent_spaces = if indent_px >= char_w * 0.55 {
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
        };

        let mut line_str = String::new();
        if indent_spaces > 0 {
            line_str.push_str(&" ".repeat(indent_spaces));
        }

        for (i, word) in line.words.iter().enumerate() {
            if i == 0 {
                line_str.push_str(&word.text);
            } else {
                let prev = &line.words[i - 1];
                let prev_end = prev.x + prev.width;
                let gap = (word.x - prev_end).max(0.0);

                if gap < char_w * 0.35 {
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

/// Parses Tesseract TSV output into a strongly typed OcrResult with line and word bounding boxes.
pub fn parse_tesseract_tsv(tsv_text: &str) -> OcrResult {
    parse_tesseract_tsv_scaled(tsv_text, 1.0)
}

/// Parses Tesseract TSV output with coordinate scaling to map upscaled OCR back to screen pixels.
pub fn parse_tesseract_tsv_scaled(tsv_text: &str, inv_scale: f64) -> OcrResult {
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
            let word_text = parts[11].trim().to_string();
            if word_text.is_empty() {
                continue;
            }

            let word = OcrWord {
                text: word_text,
                x: left,
                y: top,
                width,
                height,
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

    let (lines, full_text, boxes) = coalesce_and_structure_lines(raw_lines);

    OcrResult {
        full_text,
        lines,
        word_count: total_words,
        boxes,
    }
}

#[cfg(target_os = "windows")]
pub fn extract_text_native(image: &image::RgbaImage) -> Result<OcrResult, String> {
    use windows::Graphics::Imaging::{BitmapPixelFormat, SoftwareBitmap};
    use windows::Media::Ocr::OcrEngine;
    use windows::Storage::Streams::DataWriter;

    // 1. High-precision preprocessing: dark theme inversion + Lanczos scaling
    let (prep_image, scale) = preprocess_for_ocr(image);

    let width = prep_image.width() as i32;
    let height = prep_image.height() as i32;

    let software_bitmap = SoftwareBitmap::Create(BitmapPixelFormat::Rgba8, width, height)
        .map_err(|e| format!("Failed to create SoftwareBitmap: {e}"))?;

    let raw_bytes = prep_image.as_raw();
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

    let engine = OcrEngine::TryCreateFromUserProfileLanguages()
        .map_err(|e| format!("Failed to initialize Windows OCR Engine: {e}"))?;

    let ocr_result = engine
        .RecognizeAsync(&software_bitmap)
        .map_err(|e| format!("Failed to start OCR recognition: {e}"))?
        .get()
        .map_err(|e| format!("OCR recognition execution failed: {e}"))?;

    let inv_scale = 1.0 / scale;
    let mut raw_lines = Vec::new();

    if let Ok(ocr_lines) = ocr_result.Lines() {
        for line in ocr_lines {
            let mut words = Vec::new();

            if let Ok(ocr_words) = line.Words() {
                for word in ocr_words {
                    let word_text = word.Text().map(|t| t.to_string()).unwrap_or_default();
                    let rect = word.BoundingRect().unwrap_or_default();
                    words.push(OcrWord {
                        text: word_text,
                        x: (rect.X as f64) * inv_scale,
                        y: (rect.Y as f64) * inv_scale,
                        width: (rect.Width as f64) * inv_scale,
                        height: (rect.Height as f64) * inv_scale,
                    });
                }
            }

            if !words.is_empty() {
                let (lx, ly, lw, lh) = compute_words_bounding_box(&words);
                let line_text = line.Text().map(|t| t.to_string()).unwrap_or_default();
                raw_lines.push(OcrLine {
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

    let (lines, full_text, boxes) = coalesce_and_structure_lines(raw_lines);
    let total_words = lines.iter().map(|l| l.words.len()).sum();

    Ok(OcrResult {
        full_text,
        lines,
        word_count: total_words,
        boxes,
    })
}

#[cfg(target_os = "linux")]
pub fn extract_text_native(image: &image::RgbaImage) -> Result<OcrResult, String> {
    use std::process::Command;

    // Use unified OCR preprocessor (handles dark mode inversion + 2x scaling)
    let (temp_image, scale) = preprocess_for_ocr(image);

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

    // Check tesseract with multi-language (por+eng), --dpi 150, or fallback to default
    let output = match Command::new("tesseract")
        .args([&temp_path_str, "stdout", "--dpi", "150", "-l", "por+eng", "tsv"])
        .output()
    {
        Ok(out) if out.status.success() => out,
        _ => Command::new("tesseract")
            .args([&temp_path_str, "stdout", "--dpi", "150", "tsv"])
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
    let result = parse_tesseract_tsv_scaled(&tsv_text, inv_scale);

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
            },
            OcrWord {
                text: "World".to_string(),
                x: 70.0,
                y: 18.0,
                width: 60.0,
                height: 19.0,
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
}
