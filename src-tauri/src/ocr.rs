use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct OcrWord {
    pub text: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct OcrLine {
    pub text: String,
    pub words: Vec<OcrWord>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct OcrResult {
    pub full_text: String,
    pub lines: Vec<OcrLine>,
    pub word_count: usize,
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

            lines.push(OcrLine {
                text: line_text,
                words,
            });
        }
    }

    Ok(OcrResult {
        full_text,
        lines,
        word_count: total_words,
    })
}

#[cfg(not(target_os = "windows"))]
pub fn extract_text_native(_image: &image::RgbaImage) -> Result<OcrResult, String> {
    Err("Native OCR engine is currently configured for Windows (Sprint 4 cross-platform Linux coming next)".to_string())
}
