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

/// Parses Tesseract TSV output into a strongly typed OcrResult with line and word bounding boxes.
pub fn parse_tesseract_tsv(tsv_text: &str) -> OcrResult {
    use std::collections::BTreeMap;

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
        let left: f64 = parts[6].trim().parse().unwrap_or(0.0);
        let top: f64 = parts[7].trim().parse().unwrap_or(0.0);
        let width: f64 = parts[8].trim().parse().unwrap_or(0.0);
        let height: f64 = parts[9].trim().parse().unwrap_or(0.0);

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

    let mut lines = Vec::new();
    let mut full_text_lines = Vec::new();
    let mut boxes = Vec::new();

    for (key, words) in line_words {
        let line_text = words
            .iter()
            .map(|w| w.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");

        if line_text.is_empty() {
            continue;
        }

        let (lx, ly, lw, lh) = if let Some(&(x, y, w, h)) = line_boxes.get(&key) {
            if w > 0.0 && h > 0.0 {
                (x, y, w, h)
            } else {
                compute_words_bounding_box(&words)
            }
        } else {
            compute_words_bounding_box(&words)
        };

        full_text_lines.push(line_text.clone());
        lines.push(OcrLine {
            text: line_text.clone(),
            x: lx,
            y: ly,
            width: lw,
            height: lh,
            words,
        });

        boxes.push(BoundingBox {
            text: line_text,
            x: lx,
            y: ly,
            width: lw,
            height: lh,
            translated_text: None,
        });
    }

    let full_text = full_text_lines.join("\n");

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

    let width = image.width() as i32;
    let height = image.height() as i32;

    let software_bitmap = SoftwareBitmap::Create(BitmapPixelFormat::Rgba8, width, height)
        .map_err(|e| format!("Failed to create SoftwareBitmap: {e}"))?;

    let raw_bytes = image.as_raw();
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

    let full_text = ocr_result
        .Text()
        .map_err(|e| format!("Failed to read OCR text: {e}"))?
        .to_string();

    let mut lines = Vec::new();
    let mut boxes = Vec::new();
    let mut total_words = 0;

    if let Ok(ocr_lines) = ocr_result.Lines() {
        for line in ocr_lines {
            let line_text = line.Text().map(|t| t.to_string()).unwrap_or_default();
            let mut words = Vec::new();

            if let Ok(ocr_words) = line.Words() {
                for word in ocr_words {
                    let word_text = word.Text().map(|t| t.to_string()).unwrap_or_default();
                    let rect = word.BoundingRect().unwrap_or_default();
                    words.push(OcrWord {
                        text: word_text,
                        x: rect.X as f64,
                        y: rect.Y as f64,
                        width: rect.Width as f64,
                        height: rect.Height as f64,
                    });
                    total_words += 1;
                }
            }

            let (lx, ly, lw, lh) = compute_words_bounding_box(&words);

            lines.push(OcrLine {
                text: line_text.clone(),
                x: lx,
                y: ly,
                width: lw,
                height: lh,
                words,
            });

            boxes.push(BoundingBox {
                text: line_text,
                x: lx,
                y: ly,
                width: lw,
                height: lh,
                translated_text: None,
            });
        }
    }

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

    let (w, h) = (image.width(), image.height());
    // On desktop screens, text is rendered at ~96 DPI (often only 8-16px per character).
    // Tesseract neural models expect ~300 DPI text (30-60px per character).
    // Upscaling by 2x with Lanczos interpolation and passing --dpi 150
    // yields a massive increase in OCR recall and accuracy.
    let scale: f64 = if w < 1600 && h < 1600 { 2.0 } else { 1.0 };
    let temp_image = if scale > 1.0 {
        let new_w = (w as f64 * scale).round() as u32;
        let new_h = (h as f64 * scale).round() as u32;
        image::imageops::resize(image, new_w, new_h, image::imageops::FilterType::Lanczos3)
    } else {
        image.clone()
    };

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
    let mut result = parse_tesseract_tsv(&tsv_text);

    // If image was upscaled, map coordinates back to original unscaled screen pixels
    if scale != 1.0 {
        let inv_scale = 1.0 / scale;
        for line in &mut result.lines {
            line.x *= inv_scale;
            line.y *= inv_scale;
            line.width *= inv_scale;
            line.height *= inv_scale;
            for word in &mut line.words {
                word.x *= inv_scale;
                word.y *= inv_scale;
                word.width *= inv_scale;
                word.height *= inv_scale;
            }
        }
        for bbox in &mut result.boxes {
            bbox.x *= inv_scale;
            bbox.y *= inv_scale;
            bbox.width *= inv_scale;
            bbox.height *= inv_scale;
        }
    }

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
