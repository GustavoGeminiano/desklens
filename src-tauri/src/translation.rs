use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Input bounding box matching OCR coordinates
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TranslationBoxInput {
    pub text: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Output bounding box with translated text
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TranslationBoxOutput {
    pub text: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    #[serde(rename = "translated_text", alias = "translatedText")]
    pub translated_text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub is_paragraph: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub detected_source_lang: Option<String>,
}

/// Structured translation response containing output boxes and detected source language
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TranslationResponse {
    pub boxes: Vec<TranslationBoxOutput>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub detected_source_lang: Option<String>,
}

impl std::ops::Deref for TranslationResponse {
    type Target = [TranslationBoxOutput];

    fn deref(&self) -> &Self::Target {
        &self.boxes
    }
}

impl std::ops::DerefMut for TranslationResponse {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.boxes
    }
}

impl std::ops::Index<usize> for TranslationResponse {
    type Output = TranslationBoxOutput;

    fn index(&self, index: usize) -> &Self::Output {
        &self.boxes[index]
    }
}

impl std::ops::IndexMut<usize> for TranslationResponse {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.boxes[index]
    }
}

impl IntoIterator for TranslationResponse {
    type Item = TranslationBoxOutput;
    type IntoIter = std::vec::IntoIter<TranslationBoxOutput>;

    fn into_iter(self) -> Self::IntoIter {
        self.boxes.into_iter()
    }
}

impl<'a> IntoIterator for &'a TranslationResponse {
    type Item = &'a TranslationBoxOutput;
    type IntoIter = std::slice::Iter<'a, TranslationBoxOutput>;

    fn into_iter(self) -> Self::IntoIter {
        self.boxes.iter()
    }
}

impl PartialEq<Vec<TranslationBoxOutput>> for TranslationResponse {
    fn eq(&self, other: &Vec<TranslationBoxOutput>) -> bool {
        &self.boxes == other
    }
}

impl PartialEq<TranslationResponse> for Vec<TranslationBoxOutput> {
    fn eq(&self, other: &TranslationResponse) -> bool {
        self == &other.boxes
    }
}

impl From<Vec<TranslationBoxOutput>> for TranslationResponse {
    fn from(boxes: Vec<TranslationBoxOutput>) -> Self {
        Self {
            boxes,
            detected_source_lang: None,
        }
    }
}

impl TranslationResponse {
    pub fn new(boxes: Vec<TranslationBoxOutput>, detected_source_lang: Option<String>) -> Self {
        Self {
            boxes,
            detected_source_lang,
        }
    }
}

/// Normalizes target language code (e.g. "pt-BR" -> "pt", default "pt")
pub fn normalize_target_lang(lang: Option<&str>) -> String {
    match lang.map(|s| s.trim()).filter(|s| !s.is_empty()) {
        None => "pt".to_string(),
        Some(s) => {
            let lower = s.to_lowercase();
            if lower.starts_with("pt") {
                "pt".to_string()
            } else if lower.starts_with("en") {
                "en".to_string()
            } else if lower.starts_with("es") {
                "es".to_string()
            } else if lower.len() >= 2 {
                lower.chars().take(2).collect::<String>()
            } else {
                lower
            }
        }
    }
}

/// Normalizes source language code (e.g. None or "" or "auto" -> "auto", "EN" -> "en")
pub fn normalize_source_lang(lang: Option<&str>) -> String {
    match lang.map(|s| s.trim()).filter(|s| !s.is_empty()) {
        None => "auto".to_string(),
        Some(s) => {
            let lower = s.to_lowercase();
            if lower == "auto" {
                "auto".to_string()
            } else if lower.starts_with("pt") {
                "pt".to_string()
            } else if lower.starts_with("en") {
                "en".to_string()
            } else if lower.starts_with("es") {
                "es".to_string()
            } else if lower.len() >= 2 {
                lower.chars().take(2).collect::<String>()
            } else {
                lower
            }
        }
    }
}

/// Simple URL encoder for query parameters (RFC 3986)
pub fn url_encode(input: &str) -> String {
    let mut encoded = String::with_capacity(input.len() * 3);
    for byte in input.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            _ => {
                encoded.push_str(&format!("%{:02X}", byte));
            }
        }
    }
    encoded
}

/// Clusters translation boxes into paragraph units. Returns (grouped_boxes, is_paragraph_flags).
pub fn cluster_translation_boxes(
    boxes: &[TranslationBoxInput],
) -> (Vec<TranslationBoxInput>, Vec<bool>) {
    if boxes.len() <= 1 {
        let is_para = vec![false; boxes.len()];
        return (boxes.to_vec(), is_para);
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

    let mut grouped = Vec::new();
    let mut is_para_flags = Vec::new();
    let mut current_indices: Vec<usize> = vec![0];

    for j in 1..boxes.len() {
        let last_idx = *current_indices.last().unwrap();
        let curr = &boxes[last_idx];
        let next = &boxes[j];

        let should_merge = crate::ocr::should_merge_lines_into_paragraph(
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
        );

        if should_merge {
            current_indices.push(j);
        } else {
            let is_para = current_indices.len() > 1;
            grouped.push(build_grouped_input_box(boxes, &current_indices));
            is_para_flags.push(is_para);
            current_indices = vec![j];
        }
    }

    if !current_indices.is_empty() {
        let is_para = current_indices.len() > 1;
        grouped.push(build_grouped_input_box(boxes, &current_indices));
        is_para_flags.push(is_para);
    }

    (grouped, is_para_flags)
}

/// Groups a slice of TranslationBoxInput into cohesive paragraph boxes when they form
/// natural multi-line text paragraphs. Standalone lines, code, and labels are preserved as-is.
pub fn group_translation_boxes_into_paragraphs(
    boxes: &[TranslationBoxInput],
) -> Vec<TranslationBoxInput> {
    cluster_translation_boxes(boxes).0
}

fn build_grouped_input_box(
    boxes: &[TranslationBoxInput],
    indices: &[usize],
) -> TranslationBoxInput {
    if indices.len() == 1 {
        return boxes[indices[0]].clone();
    }

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
    let text = crate::ocr::join_paragraph_lines(&line_texts);

    TranslationBoxInput {
        text,
        x: min_x,
        y: min_y,
        width,
        height,
    }
}

/// Translates a single chunk of texts using Google Translate dict-chrome-ex endpoint.
/// Returns translated strings and detected source language.
async fn translate_chunk_google(
    client: &reqwest::Client,
    texts: &[String],
    target_lang: &str,
    source_lang: &str,
) -> Result<(Vec<String>, Option<String>), String> {
    if texts.is_empty() {
        return Ok((Vec::new(), None));
    }

    let url = "https://clients5.google.com/translate_a/t?client=dict-chrome-ex";

    // Manually format application/x-www-form-urlencoded payload:
    // sl=<source_lang>&tl=<target_lang>&q=<text1>&q=<text2>...
    let mut parts: Vec<String> = Vec::with_capacity(texts.len() + 2);
    parts.push(format!("sl={}", url_encode(source_lang)));
    parts.push(format!("tl={}", url_encode(target_lang)));
    for text in texts {
        parts.push(format!("q={}", url_encode(text)));
    }
    let body_str = parts.join("&");

    let response = client
        .post(url)
        .header(
            "User-Agent",
            "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/128.0.0.0 Safari/537.36",
        )
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body_str)
        .timeout(Duration::from_millis(1800))
        .send()
        .await
        .map_err(|e| format!("HTTP request failed: {e}"))?;

    if !response.status().is_success() {
        return Err(format!("Google Translate HTTP status: {}", response.status()));
    }

    let body = response
        .text()
        .await
        .map_err(|e| format!("Failed to read response body: {e}"))?;

    let json: serde_json::Value = serde_json::from_str(&body)
        .map_err(|e| format!("JSON parse failed: {e} - body: {body}"))?;

    let array = json
        .as_array()
        .ok_or_else(|| "Google Translate response is not an array".to_string())?;

    let mut translated = Vec::with_capacity(texts.len());
    let mut detected_lang: Option<String> = None;

    for (idx, item) in array.iter().enumerate() {
        if let Some(pair) = item.as_array() {
            if let Some(trans_str) = pair.first().and_then(|v| v.as_str()) {
                if trans_str.trim().is_empty() {
                    translated.push(texts.get(idx).cloned().unwrap_or_default());
                } else {
                    translated.push(trans_str.to_string());
                }

                if detected_lang.is_none() {
                    if let Some(det) = pair.get(1).and_then(|v| v.as_str()) {
                        let det_clean = det.trim();
                        if !det_clean.is_empty() {
                            detected_lang = Some(det_clean.to_lowercase());
                        }
                    }
                }
                continue;
            }
        } else if let Some(trans_str) = item.as_str() {
            if trans_str.trim().is_empty() {
                translated.push(texts.get(idx).cloned().unwrap_or_default());
            } else {
                translated.push(trans_str.to_string());
            }
            continue;
        }
        translated.push(texts.get(idx).cloned().unwrap_or_default());
    }

    while translated.len() < texts.len() {
        translated.push(texts[translated.len()].clone());
    }

    Ok((translated, detected_lang))
}

/// Fallback for translating a single text via MyMemory API with bounded timeout
async fn translate_single_mymemory(
    client: &reqwest::Client,
    text: &str,
    target_lang: &str,
    source_lang: &str,
) -> Result<String, String> {
    let sl = if source_lang == "auto" { "autodetect" } else { source_lang };
    let url = format!(
        "https://api.mymemory.translated.net/get?q={}&langpair={}|{}",
        url_encode(text),
        url_encode(sl),
        url_encode(target_lang)
    );

    let response = client
        .get(&url)
        .timeout(Duration::from_millis(1000))
        .send()
        .await
        .map_err(|e| format!("MyMemory request error: {e}"))?;

    if !response.status().is_success() {
        return Err(format!("MyMemory HTTP status: {}", response.status()));
    }

    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("MyMemory JSON error: {e}"))?;

    if let Some(trans) = json["responseData"]["translatedText"].as_str() {
        if !trans.trim().is_empty() {
            return Ok(trans.to_string());
        }
    }

    Err("MyMemory translation empty".to_string())
}

/// DeepL API translation for Pro/API users
async fn translate_batch_deepl(
    client: &reqwest::Client,
    texts: &[String],
    target_lang: &str,
    source_lang: &str,
    api_key: &str,
) -> Result<(Vec<String>, Option<String>), String> {
    let endpoint = if api_key.ends_with(":fx") {
        "https://api-free.deepl.com/v2/translate"
    } else {
        "https://api.deepl.com/v2/translate"
    };

    let target_deepl = if target_lang.eq_ignore_ascii_case("pt") {
        "PT-BR"
    } else {
        target_lang
    };

    let mut payload = serde_json::json!({
        "text": texts,
        "target_lang": target_deepl
    });

    if source_lang != "auto" {
        payload["source_lang"] = serde_json::Value::String(source_lang.to_uppercase());
    }

    let resp = client
        .post(endpoint)
        .header("Authorization", format!("DeepL-Auth-Key {api_key}"))
        .header("Content-Type", "application/json")
        .json(&payload)
        .timeout(Duration::from_secs(5))
        .send()
        .await
        .map_err(|e| format!("DeepL request failed: {e}"))?;

    let json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("DeepL JSON parse failed: {e}"))?;

    if let Some(translations) = json["translations"].as_array() {
        let mut results = Vec::with_capacity(texts.len());
        let mut detected_lang: Option<String> = None;
        for (i, item) in translations.iter().enumerate() {
            let trans = item["text"].as_str().unwrap_or_else(|| &texts[i]);
            results.push(trans.to_string());
            if detected_lang.is_none() {
                if let Some(det) = item["detected_source_language"].as_str() {
                    let d = det.trim().to_lowercase();
                    if !d.is_empty() {
                        detected_lang = Some(d);
                    }
                }
            }
        }
        return Ok((results, detected_lang));
    }

    Err("Invalid DeepL response structure".to_string())
}

/// Prepared structure for Gemini API translation
async fn translate_batch_gemini(
    client: &reqwest::Client,
    texts: &[String],
    target_lang: &str,
    source_lang: &str,
    api_key: &str,
) -> Result<(Vec<String>, Option<String>), String> {
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/gemini-1.5-flash:generateContent?key={}",
        api_key
    );

    let prompt = if source_lang == "auto" {
        format!(
            "Translate each of the following lines into language code '{}'. Return ONLY a JSON array of strings in identical order without markdown code fences:\n{}",
            target_lang,
            serde_json::to_string(texts).map_err(|e| e.to_string())?
        )
    } else {
        format!(
            "Translate each of the following lines from language code '{}' into language code '{}'. Return ONLY a JSON array of strings in identical order without markdown code fences:\n{}",
            source_lang,
            target_lang,
            serde_json::to_string(texts).map_err(|e| e.to_string())?
        )
    };

    let payload = serde_json::json!({
        "contents": [{
            "parts": [{ "text": prompt }]
        }]
    });

    let resp = client
        .post(&url)
        .header("Content-Type", "application/json")
        .json(&payload)
        .timeout(Duration::from_secs(5))
        .send()
        .await
        .map_err(|e| format!("Gemini request failed: {e}"))?;

    let json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Gemini JSON parse error: {e}"))?;

    if let Some(raw_text) = json["candidates"][0]["content"]["parts"][0]["text"].as_str() {
        let trimmed = raw_text
            .trim()
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim();
        if let Ok(parsed) = serde_json::from_str::<Vec<String>>(trimmed) {
            if parsed.len() == texts.len() {
                let det = if source_lang != "auto" {
                    Some(source_lang.to_string())
                } else {
                    None
                };
                return Ok((parsed, det));
            }
        }
    }

    Err("Gemini API parsing failed".to_string())
}

/// Batch chunks runner with parallel bounded fallback chain
async fn translate_all_chunks(
    client: &reqwest::Client,
    texts: &[String],
    target_lang: &str,
    source_lang: &str,
) -> (Vec<String>, Option<String>) {
    const CHUNK_SIZE: usize = 30;
    let mut results = Vec::with_capacity(texts.len());
    let mut detected_source_lang: Option<String> = None;
    let start_time = std::time::Instant::now();
    let max_budget = std::time::Duration::from_millis(2800);

    for chunk in texts.chunks(CHUNK_SIZE) {
        if start_time.elapsed() >= max_budget {
            eprintln!("⚠️ [DeskLens] Translation timeout budget exceeded. Using original text for remaining chunks.");
            results.extend(chunk.iter().cloned());
            continue;
        }

        match translate_chunk_google(client, chunk, target_lang, source_lang).await {
            Ok((translated_chunk, detected)) => {
                if detected_source_lang.is_none() && detected.is_some() {
                    detected_source_lang = detected;
                }
                results.extend(translated_chunk);
            }
            Err(err) => {
                eprintln!(
                    "⚠️ [DeskLens] Google batch translation failed ({err}). Trying bounded parallel fallback..."
                );
                let remaining = max_budget.saturating_sub(start_time.elapsed());
                if remaining.is_zero() {
                    results.extend(chunk.iter().cloned());
                    continue;
                }

                let mut handles = Vec::with_capacity(chunk.len());
                for item in chunk {
                    let text = item.clone();
                    if text.trim().is_empty() {
                        handles.push(tauri::async_runtime::spawn(async move { text }));
                        continue;
                    }
                    let client = client.clone();
                    let target = target_lang.to_string();
                    let source = source_lang.to_string();
                    handles.push(tauri::async_runtime::spawn(async move {
                        match translate_single_mymemory(&client, &text, &target, &source).await {
                            Ok(fallback_text) => fallback_text,
                            Err(_) => text,
                        }
                    }));
                }

                for handle in handles {
                    match handle.await {
                        Ok(text) => results.push(text),
                        Err(_) => results.push(String::new()),
                    }
                }
            }
        }
    }

    (results, detected_source_lang)
}

/// SPRINT R2 / M1: Translates on-screen bounding boxes into target language (defaults to Portuguese).
/// Never crashes or panics: falls back to original text on any network failure or timeout.
/// Groups adjacent lines into cohesive paragraphs so MT receives complete semantic sentences.
#[tauri::command]
pub async fn translate_on_screen(
    boxes: Vec<TranslationBoxInput>,
    target_lang: Option<String>,
    source_lang: Option<String>,
) -> Result<TranslationResponse, String> {
    if boxes.is_empty() {
        return Ok(TranslationResponse::new(Vec::new(), None));
    }

    let target = normalize_target_lang(target_lang.as_deref());
    let source = normalize_source_lang(source_lang.as_deref());
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_millis(1500))
        .timeout(Duration::from_millis(2800))
        .build()
        .unwrap_or_default();

    // Group adjacent lines into cohesive paragraphs when appropriate
    let (grouped_inputs, is_para_flags) = cluster_translation_boxes(&boxes);
    let texts: Vec<String> = grouped_inputs.iter().map(|b| b.text.clone()).collect();

    // Check if user has DEEPL_API_KEY or GEMINI_API_KEY configured in environment
    let (translated_texts, mut detected_source_lang) = if let Ok(deepl_key) = std::env::var("DEEPL_API_KEY") {
        if !deepl_key.trim().is_empty() {
            translate_batch_deepl(&client, &texts, &target, &source, &deepl_key)
                .await
                .unwrap_or_else(|e| {
                    eprintln!("⚠️ [DeskLens] DeepL API failed ({e}), falling back to public endpoint.");
                    (Vec::new(), None)
                })
        } else {
            (Vec::new(), None)
        }
    } else if let Ok(gemini_key) = std::env::var("GEMINI_API_KEY") {
        if !gemini_key.trim().is_empty() {
            translate_batch_gemini(&client, &texts, &target, &source, &gemini_key)
                .await
                .unwrap_or_else(|e| {
                    eprintln!("⚠️ [DeskLens] Gemini API failed ({e}), falling back to public endpoint.");
                    (Vec::new(), None)
                })
        } else {
            (Vec::new(), None)
        }
    } else {
        (Vec::new(), None)
    };

    let (final_translated, detected) = if translated_texts.len() == texts.len() {
        (translated_texts, detected_source_lang)
    } else {
        let (res, det) = translate_all_chunks(&client, &texts, &target, &source).await;
        if detected_source_lang.is_none() {
            detected_source_lang = det.clone();
        }
        (res, detected_source_lang)
    };

    let outputs: Vec<TranslationBoxOutput> = grouped_inputs
        .into_iter()
        .zip(final_translated.into_iter())
        .zip(is_para_flags.into_iter())
        .map(|((input_box, trans_text), is_para)| {
            TranslationBoxOutput {
                text: input_box.text,
                x: input_box.x,
                y: input_box.y,
                width: input_box.width,
                height: input_box.height,
                translated_text: trans_text,
                is_paragraph: if is_para { Some(true) } else { None },
                detected_source_lang: detected.clone(),
            }
        })
        .collect();

    println!(
        "🌐 [DeskLens] Translated {} text blocks ({} -> {}) [detected: {:?}].",
        outputs.len(),
        source,
        target,
        detected
    );

    Ok(TranslationResponse::new(outputs, detected))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_target_lang() {
        assert_eq!(normalize_target_lang(None), "pt");
        assert_eq!(normalize_target_lang(Some("")), "pt");
        assert_eq!(normalize_target_lang(Some("   ")), "pt");
        assert_eq!(normalize_target_lang(Some("pt-BR")), "pt");
        assert_eq!(normalize_target_lang(Some("PT")), "pt");
        assert_eq!(normalize_target_lang(Some("en-US")), "en");
        assert_eq!(normalize_target_lang(Some("es-ES")), "es");
        assert_eq!(normalize_target_lang(Some("fr")), "fr");
    }

    #[test]
    fn test_normalize_source_lang() {
        assert_eq!(normalize_source_lang(None), "auto");
        assert_eq!(normalize_source_lang(Some("")), "auto");
        assert_eq!(normalize_source_lang(Some("   ")), "auto");
        assert_eq!(normalize_source_lang(Some("auto")), "auto");
        assert_eq!(normalize_source_lang(Some("AUTO")), "auto");
        assert_eq!(normalize_source_lang(Some("en-US")), "en");
        assert_eq!(normalize_source_lang(Some("pt-BR")), "pt");
        assert_eq!(normalize_source_lang(Some("es")), "es");
        assert_eq!(normalize_source_lang(Some("ja")), "ja");
    }

    #[test]
    fn test_url_encode() {
        assert_eq!(url_encode("Hello World"), "Hello%20World");
        assert_eq!(url_encode("foo_bar-1.2~"), "foo_bar-1.2~");
        assert_eq!(url_encode("a+b=c&d"), "a%2Bb%3Dc%26d");
    }

    #[test]
    fn test_translate_empty_boxes() {
        tauri::async_runtime::block_on(async {
            let result = translate_on_screen(vec![], None, None).await;
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), vec![]);
        });
    }
}
