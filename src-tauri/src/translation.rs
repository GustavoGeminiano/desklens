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

/// Translates a single chunk of texts using Google Translate dict-chrome-ex endpoint
async fn translate_chunk_google(
    client: &reqwest::Client,
    texts: &[String],
    target_lang: &str,
    source_lang: &str,
) -> Result<Vec<String>, String> {
    if texts.is_empty() {
        return Ok(Vec::new());
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
        .timeout(Duration::from_secs(5))
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
    for (idx, item) in array.iter().enumerate() {
        if let Some(pair) = item.as_array() {
            if let Some(trans_str) = pair.first().and_then(|v| v.as_str()) {
                if trans_str.trim().is_empty() {
                    translated.push(texts.get(idx).cloned().unwrap_or_default());
                } else {
                    translated.push(trans_str.to_string());
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

    Ok(translated)
}

/// Fallback for translating a single text via MyMemory API
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
        .timeout(Duration::from_secs(4))
        .send()
        .await
        .map_err(|e| format!("MyMemory request error: {e}"))?;

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
) -> Result<Vec<String>, String> {
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
        for (i, item) in translations.iter().enumerate() {
            let trans = item["text"].as_str().unwrap_or_else(|| &texts[i]);
            results.push(trans.to_string());
        }
        return Ok(results);
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
) -> Result<Vec<String>, String> {
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
        let trimmed = raw_text.trim().trim_start_matches("```json").trim_start_matches("```").trim_end_matches("```").trim();
        if let Ok(parsed) = serde_json::from_str::<Vec<String>>(trimmed) {
            if parsed.len() == texts.len() {
                return Ok(parsed);
            }
        }
    }

    Err("Gemini API parsing failed".to_string())
}

/// Batch chunks runner with comprehensive fallback chain
async fn translate_all_chunks(
    client: &reqwest::Client,
    texts: &[String],
    target_lang: &str,
    source_lang: &str,
) -> Vec<String> {
    const CHUNK_SIZE: usize = 30;
    let mut results = Vec::with_capacity(texts.len());

    for chunk in texts.chunks(CHUNK_SIZE) {
        match translate_chunk_google(client, chunk, target_lang, source_lang).await {
            Ok(translated_chunk) => {
                results.extend(translated_chunk);
            }
            Err(err) => {
                eprintln!("⚠️ [DeskLens] Google batch translation failed ({err}). Trying individual fallback...");
                for item in chunk {
                    if item.trim().is_empty() {
                        results.push(item.clone());
                        continue;
                    }
                    match translate_single_mymemory(client, item, target_lang, source_lang).await {
                        Ok(fallback_text) => results.push(fallback_text),
                        Err(_) => results.push(item.clone()),
                    }
                }
            }
        }
    }

    results
}

/// SPRINT R2: Translates on-screen bounding boxes into target language (defaults to Portuguese).
/// Never crashes or panics: falls back to original text on any network failure or timeout.
#[tauri::command]
pub async fn translate_on_screen(
    boxes: Vec<TranslationBoxInput>,
    target_lang: Option<String>,
    source_lang: Option<String>,
) -> Result<Vec<TranslationBoxOutput>, String> {
    if boxes.is_empty() {
        return Ok(Vec::new());
    }

    let target = normalize_target_lang(target_lang.as_deref());
    let source = normalize_source_lang(source_lang.as_deref());
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap_or_default();

    let texts: Vec<String> = boxes.iter().map(|b| b.text.clone()).collect();

    // Check if user has DEEPL_API_KEY or GEMINI_API_KEY configured in environment
    let translated_texts = if let Ok(deepl_key) = std::env::var("DEEPL_API_KEY") {
        if !deepl_key.trim().is_empty() {
            translate_batch_deepl(&client, &texts, &target, &source, &deepl_key)
                .await
                .unwrap_or_else(|e| {
                    eprintln!("⚠️ [DeskLens] DeepL API failed ({e}), falling back to public endpoint.");
                    Vec::new()
                })
        } else {
            Vec::new()
        }
    } else if let Ok(gemini_key) = std::env::var("GEMINI_API_KEY") {
        if !gemini_key.trim().is_empty() {
            translate_batch_gemini(&client, &texts, &target, &source, &gemini_key)
                .await
                .unwrap_or_else(|e| {
                    eprintln!("⚠️ [DeskLens] Gemini API failed ({e}), falling back to public endpoint.");
                    Vec::new()
                })
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };

    let final_translated = if translated_texts.len() == texts.len() {
        translated_texts
    } else {
        translate_all_chunks(&client, &texts, &target, &source).await
    };

    let outputs: Vec<TranslationBoxOutput> = boxes
        .into_iter()
        .zip(final_translated.into_iter())
        .map(|(input_box, trans_text)| TranslationBoxOutput {
            text: input_box.text,
            x: input_box.x,
            y: input_box.y,
            width: input_box.width,
            height: input_box.height,
            translated_text: trans_text,
        })
        .collect();

    println!(
        "🌐 [DeskLens] Translated {} text blocks ({} -> {}).",
        outputs.len(),
        source,
        target
    );

    Ok(outputs)
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
